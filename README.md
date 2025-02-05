## PicoVault

Minimal Redis-like KV Store Implementation in Rust, using tokio and mutexes. Part of my journey of learning & building in rust.

To run locally,
 - **Clone the repository:** `git clone https://github.com/Shiva953/Picovault.git`
 - **CD into the repository:** `cd picovault`
 - **Start the server:** `./spawn_redis_server.sh`

While the server is running, you can try out the following commands:

### Commands

- `ECHO, PING`

  <img width="642" alt="Screenshot 2025-02-05 at 9 35 23 PM" src="https://github.com/user-attachments/assets/ff5e8d2f-fe0f-437c-875b-c9b919e3d694" />
  <img width="704" alt="Screenshot 2025-02-05 at 9 36 38 PM" src="https://github.com/user-attachments/assets/6483c9b3-2458-4cb3-b5a1-4b9e76191c97" />


- Key-Value Store `[GET, SET]`
  
  <img width="829" alt="Screenshot 2025-02-05 at 9 36 09 PM" src="https://github.com/user-attachments/assets/cb45d2f1-c844-493a-9558-ba2a43b63e9f" />

  
- `INCR(increment)`
- 
  <img width="810" alt="Screenshot 2025-02-05 at 9 38 31 PM" src="https://github.com/user-attachments/assets/2633c6a4-c947-40c2-87b6-ab88e97aa1d0" />

  
- `REPLICA ➔ MASTER`
- `REPLCONF, PSYNC`

