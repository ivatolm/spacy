use std::{thread, sync::{mpsc::{self, Receiver, Sender}, Arc, Mutex}, net::{TcpListener, TcpStream, Shutdown}, collections::HashMap, io::{Read, Write}};
use log::{info, debug, warn, trace};
use num_derive::FromPrimitive;
use common::{event::Event, tools, message, events::MCEvents};
use num_traits::FromPrimitive;
use crate::{fsm::StateMachine};
use super::handler_fsm_data::{ClientHandlerState, gen_transitions};

#[derive(FromPrimitive, Clone, Copy)]
enum SelfEvents {
  NewConnection = 0,
  AddPlugin = 1
}

pub struct ClientHandler {
  fsm: StateMachine,
  clients: Arc<Mutex<HashMap<u8, TcpStream>>>
}

impl ClientHandler {
  pub fn new() -> Self {
    let fsm = StateMachine::new(ClientHandlerState::Init as u8, gen_transitions());

    Self { fsm, clients: Arc::new(Mutex::new(HashMap::new())) }
  }

  pub fn start(self, port: u16, main_tx: Sender<Event>) {
    let (handler_tx, handler_rx) = mpsc::channel();

    let handler_tx_clone = handler_tx.clone();
    let clients_clone = self.clients.clone();
    debug!("Spawning server...");
    thread::spawn(move || Self::listener(port, handler_tx_clone, clients_clone));

    debug!("Spawning fsm...");
    thread::spawn(move || self.fsm(handler_tx, handler_rx, main_tx));
  }

  fn fsm(mut self, tx: Sender<Event>, rx: Receiver<Event>, main_tx: Sender<Event>) {
    loop {
      self.reader(tx.clone());

      let state: ClientHandlerState = self.fsm.state.try_into().unwrap();

      match state {
        ClientHandlerState::Err => {
          trace!("State: `err`");

          warn!("Error occured!");
          panic!();
        },
        ClientHandlerState::Init => {
          trace!("State: `init`");

          trace!("Transitioning into `wait_event`");
          self.fsm.transition(ClientHandlerState::WaitEvent as u8).unwrap();
        },
        ClientHandlerState::WaitEvent => {
          trace!("State: `wait_event`");

          let event = match rx.recv() {
            Ok(event) => event,
            Err(_) => {
              trace!("Transitioning into `err`");
              self.fsm.transition(ClientHandlerState::Err as u8).unwrap();
              continue;
            }
          };

          match FromPrimitive::from_u8(event.kind) {
            Some(SelfEvents::NewConnection) => {
              trace!("Handling `NewConnection` event...");

              info!("New client connected");
            },
            Some(SelfEvents::AddPlugin) => {
              trace!("Handling `AddPlugin` event...");

              let client_id = event.meta.get(0).unwrap();
              let message = message::proto_msg::Message {
                cmd: Some(message::proto_msg::message::Cmd::Response as i32),
                data: vec![]
              };

              let event = Event::new(MCEvents::AddPlugin as u8, event.data, vec![]);
              main_tx.send(event).unwrap();

              self.reply(client_id, message);
            },
            None => todo!()
          }

          debug!("ClientHandler received new event: {}", event.kind.to_string());
        }
      }
    }
  }

  fn reply(&mut self, client_id: &u8, message: message::proto_msg::Message) {
    let clients = self.clients.lock().unwrap();
    let mut stream = clients.get(&client_id).unwrap();

    stream.write(&message::serialize_message(message)).unwrap();
  }

  fn reader(&self, tx: Sender<Event>) {
    // reads from all client connections
    // creates specific events and sends it to the handler
    let mut clients = self.clients.lock().unwrap();

    let mut disconnected = vec![];
    for (key, mut stream) in clients.iter() {
      let mut buf = [0u8; 16384];
      let size = stream.read(&mut buf).unwrap();

      if size == 0 {
        stream.shutdown(Shutdown::Both).unwrap();
        disconnected.push(key.clone());
        continue;
      }

      // temporary solution
      let add_plugin = message::proto_msg::message::Cmd::AddPlugin;

      let msg = message::deserialize_message(&buf[..size]).unwrap();
      let cmd = msg.cmd.unwrap();

      let kind = match cmd {
        add_plugin => {
          SelfEvents::AddPlugin
        },
        _ => {
          warn!("Client sent unknown command");
          stream.shutdown(Shutdown::Both).unwrap();
          disconnected.push(key.clone());
          continue;
        }
      };

      let data = msg.data.iter()
        .map(|x| x.as_bytes().to_vec())
        .collect();

      let event = Event::new(kind as u8, data, vec![*key]);
      tx.send(event).unwrap();
    }

    for client_id in disconnected.iter() {
      clients.remove(&client_id);
    }
  }

  fn listener(port: u16, tx: Sender<Event>, clients: Arc<Mutex<HashMap<u8, TcpStream>>>) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();
    for stream in listener.incoming() {
      debug!("New client connected");
      let stream = stream.unwrap();

      let mut id = tools::gen_random_id();
      while {
        let clients = clients.lock().unwrap();
        clients.contains_key(&id)
      } {
        id = tools::gen_random_id();
      }
      debug!("Generated client-id: {}", id);

      let mut clients = clients.lock().unwrap();
      clients.insert(id, stream);

      trace!("Sending an event to event-handler...");
      let event = Event::new(SelfEvents::NewConnection as u8, vec![], vec![id]);
      tx.send(event).unwrap();
    }
  }
}
