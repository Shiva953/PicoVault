// use std::{
//     io::{Read, Write},
// };
use redis_starter_rust::ThreadPool;
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::{Arc, Mutex};
use resp::Value;

mod resp;

#[tokio::main]
async fn main() {
    // listening for connections
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    //creating a global k-v store, which gets updated each time a client(prev/new) adds a new (k,v)
    //Arc is used 
    let kv_store: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::<String,String>::new()));
    // for stream in listener.incoming() { 
        //HANDLING CONCURRENT CLIENTS, NEW THREAD FOR EACH CLIENTi/SERVER stream
        loop{ //INSTEAD OF USING for stream in listener.incoming() and synchronously iterating over each stream, we are asynchronously iterating over each stream till the data from the buffer ends
        let stream = listener.accept().await; // listener.accept().await ASYNCHRONOUSLY WAITS FOR A NEW CONNECTION, INSTEAD OF AN SYNCHRONOUS ITERATOR LIKE listener.incoming() which takes each new connection and puts inside it
        let mut kv_store = Arc::clone(&kv_store);
        match stream { 
            Ok((stream, _)) => {
                //SPAWNING A NEW THREAD FOR EACH CLIENT REQ->S
                //tried using threadpool and pool.execute, turns out each thread in it was unable to handle ASYNC read/write tasks
                //the below spawns a new ASYNC THREAD for each new client request to the redis server
                tokio::spawn(async move{ 
                    handle_conn(stream, &mut kv_store).await;
                });

                //ECHO print command
            }
            Err(e) => {
                println!("{e}");
            }
        }
    }
}

async fn handle_conn(stream: TcpStream, kv_store: &mut Arc<std::sync::Mutex<HashMap<String, String>>>) {
    let mut handler = resp::RespHandler::new(stream);
    // let mut kv_store = HashMap::<String,String>::new();
    loop{
        let value = handler.read_value().await.unwrap(); //ALL PARSING HAPPENS IN THS FUNCTION 

        let res = if let Some(v) = value {
            let (command, args) = extract_command(v).unwrap();
            match command.as_str().to_lowercase().as_str() {
                "ping" => Value::SimpleString("PONG".to_string()),
                "echo" => args.first().unwrap().clone(),
                "set" => {store_key_value(unpack_bulk_str(args[0].clone()).unwrap(), unpack_bulk_str(args[1].clone()).unwrap(), kv_store)},
                "get" => {get_value_from_key(unpack_bulk_str(args[0].clone()).unwrap(), kv_store)}, //by default, consider a input string as bulk string
                c => panic!("Cannot handle command {}", c),
            }
        } else {
            break;
        };
        handler.write_value(res).await;
    }
}
//makes sense to store in a global shared hashmap

fn store_key_value(key: String, value: String, kv_store: &mut Arc<std::sync::Mutex<HashMap<String, String>>>) -> Value{
    kv_store.lock().unwrap().insert(key, value);
    println!("{:?}", kv_store.lock().unwrap());
    Value::SimpleString("OK".to_string())
}

fn get_value_from_key(key: String, kv_store: &mut Arc<std::sync::Mutex<HashMap<String, String>>>) -> Value{
    println!("{:?}", kv_store);
    match kv_store.lock().unwrap().get(&key) {
        Some(v) => Value::BulkString(v.to_string()),
        None => Value::SimpleString("(null)".to_string())
    }
}

//extracting the command used after redis-cli, along with the args after the command[redis-cli <command> [..args]]
// returning (command, [..args])
fn extract_command(value: Value) -> Result<(String, Vec<Value>)> {
    match value {
        Value::Array(a) => { //[command, ..arguments]
            Ok((
                unpack_bulk_str(a.first().unwrap().clone())?, //command 
                a.into_iter().skip(1).collect(), //[..arguments]
            ))
        },
        _ => Err(anyhow::anyhow!("Unexpected command format")),
    }
}
fn unpack_bulk_str(value: Value) -> Result<String> {
    match value {
        Value::BulkString(s) => Ok(s),
        _ => Err(anyhow::anyhow!("Expected command to be a bulk string"))
    }
}
