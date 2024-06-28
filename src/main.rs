// use std::{
//     io::{Read, Write},
// };
// use tokio::io::{AsyncReadExt, AsyncWriteExt};
// use tokio::time::{timeout, Duration};
use core::panic;
use std::{collections::HashMap, vec};
use tokio::net::{TcpListener, TcpStream};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use resp::Value;
use std::env;
use tokio::sync::broadcast::Sender;
use tokio::sync::{broadcast};

mod resp;

struct ServerInfo {
    role: String,
    master_replid: String,
    master_repl_offset: i32
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let mut replica_info: Option<String> = None;

    let default_port = String::from("6379");
    let mut port_wrap = None;
    let port_pos = args.iter().position(|s| s=="--port");
    if let Some(pos) = port_pos{
        port_wrap = Some(args[pos + 1].clone());
    }
    let port = port_wrap.unwrap_or(default_port);
    let address = format!("127.0.0.1:{}", port);
    let mut role = "master".to_string();
    let replica_flag_position = args.iter().position(|s| s == "--replicaof");

    if let Some(pos) = replica_flag_position {
        replica_info = Some(args[pos + 1].clone());
        role = "slave".to_string();
    }
    let mut master_ip = None;
    let mut master_port = None;
    
    let server_info = Arc::new(Mutex::new(ServerInfo{role: role.clone().to_string(), master_replid: "8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb".to_string(), master_repl_offset: 0}));
   
    let listener = TcpListener::bind(address.clone()).await.unwrap(); //listening for connections in the MASTER server
    println!("Listening on port {port}");
    let (sender, mut receiver) = broadcast::channel::<String>(100);
    let sender = Arc::new(sender);
    // now the propagation
    


    //checking if replica
    if role == "slave" {
        let info = replica_info.unwrap();
        let parts: Vec<&str> = info.split_whitespace().collect();
        master_ip = Some(parts[0].to_string());
        master_port = Some(parts[1].to_string());
        println!("{:?}", format!("{}:{}", &master_ip.clone().unwrap(), &master_port.clone().unwrap()));
        // a new thread for slave -> master connection
        tokio::spawn( async move{
            match TcpStream::connect(format!("{}:{}", master_ip.unwrap().clone(), master_port.unwrap().clone())).await { //slave server connecting to master
                Ok(sockt) => {
                    println!("Slave Connected to master");
                    let mut handler = resp::RespHandler::new(sockt);
                    
                    // Send PING
                    handler.write_value(Value::BulkString("ping".to_string())).await;
                    // let res = sockt.read(&mut [0; 512]).await.unwrap();
                    // let response = timeout(Duration::from_secs(5), sockt.read(&mut [0; 512])).await;
                    let response = handler.read_value().await.unwrap();
                    println!("{:?}", response.unwrap().serialize().as_str());
                    println!("OK");
                    
        
                    // Send REPLCONF listening-port <PORT>
                    handler.write_value(Value::Array(vec![
                        Value::BulkString("REPLCONF".to_string()),
                        Value::BulkString("listening-port".to_string()),
                        Value::BulkString(port.to_string()),
                    ])).await;
                    let response = handler.read_value().await.unwrap();
                    
                    println!("Received: {:?}", response.unwrap().serialize().as_str());
        
                    // Send REPLCONF capa psync2
                    handler.write_value(Value::Array(vec![
                        Value::BulkString("REPLCONF".to_string()),
                        Value::BulkString("capa".to_string()),
                        Value::BulkString("psync2".to_string()),
                    ])).await;
                    let response = handler.read_value().await.unwrap();
                    println!("Received: {:?}", response.unwrap().serialize().as_str());
        
                    // Send PSYNC ? -1
                    handler.write_value(Value::Array(vec![
                        Value::BulkString("PSYNC".to_string()),
                        Value::BulkString("?".to_string()),
                        Value::BulkString("-1".to_string()),
                    ])).await;
                    let response = handler.read_value().await.unwrap();
                    println!("Received: {:?}", response.unwrap().serialize().as_str());

                    let received = receiver.recv().await.unwrap();
                    println!("{:?}", received);
                }
                Err(e) => {
                    println!("Failed to connect to master: {}", e);
                }
            }
        });
    }
    
        // for the command replication from master to replica, open up a new channel that sends command from client-master Thread1 -> master-replica Thread2 
        //creating a global k-v store, which gets updated each time a client(prev/new) adds a new (k,v)
        let kv_store: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::<String,String>::new()));
        //HANDLING CONCURRENT CLIENTS, NEW THREAD FOR EACH CLIENTi/SERVER stream
        loop{ //INSTEAD OF USING for stream in listener.incoming() and synchronously iterating over each stream, we are asynchronously iterating over each stream till the data from the buffer ends
        let stream = listener.accept().await; // listener.accept().await ASYNCHRONOUSLY WAITS FOR A NEW CONNECTION, INSTEAD OF AN SYNCHRONOUS ITERATOR LIKE listener.incoming() which takes each new connection and puts inside it
        let mut kv_store = Arc::clone(&kv_store);
        let mut server_store = Arc::clone(&server_info);
        let sender = sender.clone();
        
        match stream { 
            Ok((stream, _)) => {
                //SPAWNING A NEW THREAD FOR EACH CLIENT REQ->S
                //tried using threadpool and pool.execute, turns out each thread in it was unable to handle ASYNC read/write tasks
                //the below spawns a new ASYNC THREAD for each new client request to the redis server
                tokio::spawn(async move{
                    handle_conn(stream, &mut kv_store, &mut server_store, &sender).await;
                });
                
            }
            Err(e) => {
                println!("{e}");
            }
        }
    }
}

async fn handle_conn(stream: TcpStream, kv_store: &mut Arc<std::sync::Mutex<HashMap<String, String>>>, server_store: &mut Arc<std::sync::Mutex<ServerInfo>>, sender: &Arc<Sender<String>>) {
    let mut handler = resp::RespHandler::new(stream);
    // let (sender, receiver) = broadcast::channel::<String>(100);
    loop{
        let value = handler.read_value().await.unwrap(); //ALL PARSING HAPPENS IN THS FUNCTION 
        // println!("{:?}", value.as_ref().unwrap_or(&Value::SimpleString("Unknown Value".to_string())));
        
        let res = if let Some(v) = value.clone() {
            
            //this kinda assumes that whatever value must be coming must be a command
            let (command, args) = extract_command(v).unwrap();

            //rdb transfer
            // After receiving a response to the last command, the tester will expect to receive an empty RDB file from your server.
            match command.as_str().to_lowercase().as_str() {
                "ping" => Value::SimpleString("PONG".to_string()),
                "echo" => args.first().unwrap().clone(),
                "set" => {store_key_value(unpack_bulk_str(args[0].clone()).unwrap(), unpack_bulk_str(args[1].clone()).unwrap(), kv_store)},
                "get" => {get_value_from_key(unpack_bulk_str(args[0].clone()).unwrap(), kv_store)}, //by default, consider a input string as bulk string
                "info" => {get_info(unpack_bulk_str(args[0].clone()).unwrap(), server_store)},
                "replconf" => Value::SimpleString("OK".to_string()),
                "psync" => {
                    // send an empty RDB file
                    Value::Array(vec![Value::SimpleString(format!("FULL RESYNC {} 0", server_store.lock().unwrap().master_replid)), Value::BulkString(String::from_utf8_lossy(&hex::decode("524544495330303131fa0972656469732d76657205372e322e30fa0a72656469732d62697473c040fa056374696d65c26d08bc65fa08757365642d6d656dc2b0c41000fa08616f662d62617365c000fff06e3bfec0ff5aa2").unwrap()).to_string())])
                },
                c => panic!("Cannot handle command {}", c),
            }
        } else {
            break;
        };
        handler.write_value(res).await;
        sender.send(extract_command(value.clone().unwrap()).unwrap().0).unwrap();
        // handler.write_value(Value::BulkString(extract_command(value.clone().unwrap()).unwrap().0)).await;
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

// fn handle_replconf() -> Value{

// }

// fn handle_psync(server_store: &mut Arc<std::sync::Mutex<ServerInfo>>) -> Value{
//     Value::BulkString(String::from_utf8(hex::decode("UkVESVMwMDEx+glyZWRpcy12ZXIFNy4yLjD6CnJlZGlzLWJpdHPAQPoFY3RpbWXCbQi8ZfoIdXNlZC1tZW3CsMQQAPoIYW9mLWJhc2XAAP/wbjv+wP9aog==").unwrap()).unwrap().to_string())
// }

fn get_info(arg: String, server_store: &mut Arc<std::sync::Mutex<ServerInfo>>) -> Value{
    match arg.as_str(){
        "replication" => {
            let role = server_store.lock().unwrap().role.clone();
            let x = server_store.lock().unwrap().master_replid.clone();
            let y = server_store.lock().unwrap().master_repl_offset.clone();
            let info_str = format!(
                "role:{}\nmaster_replid:{}\nmaster_repl_offset:{}",
                role,
                x,
                y
            );
            Value::BulkString(info_str)
        },
        _ => Value::BulkString("Variant Not Found".to_string())
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
        Value::BulkString(x) => {
            Ok((
                x,
                Vec::new(),
            ))
        }
        _ => Err(anyhow::anyhow!("Unexpected command format")),
    }
}
fn unpack_bulk_str(value: Value) -> Result<String> {
    match value {
        Value::BulkString(s) => Ok(s),
        _ => Err(anyhow::anyhow!("Expected command to be a bulk string"))
    }
}