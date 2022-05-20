use std::{thread, sync::{mpsc::{self, Receiver, Sender}, Arc, Mutex}, net::{TcpListener, TcpStream}, collections::HashMap, io::Read};
use num_derive::FromPrimitive;
use common::{event::Event, tools};
use crate::{fsm::StateMachine};
use super::handler_fsm_data::{ClientHandlerStates, gen_transitions};

#[derive(FromPrimitive)]
enum SelfEvents {
  NewConnection = 0
}

pub struct ClientHandler {
  fsm: StateMachine,
  clients: Arc<Mutex<HashMap<u8, TcpStream>>>
}

impl ClientHandler {
  pub fn new() -> Self {
    let fsm = StateMachine::new(ClientHandlerStates::Init.to_u8(), gen_transitions());

    Self { fsm, clients: Arc::new(Mutex::new(HashMap::new())) }
  }

  pub fn start(self, port: u16) {
    let (handler_tx, handler_rx) = mpsc::channel();

    let handler_tx_clone = handler_tx.clone();
    let clients_clone = self.clients.clone();
    thread::spawn(move || Self::listener(port, handler_tx_clone, clients_clone));

    thread::spawn(move || self.fsm(handler_tx, handler_rx));
  }

  fn fsm(self, tx: Sender<Event>, rx: Receiver<Event>) {
    loop {
      let state = self.fsm.state.try_into().unwrap();

      match state {
        ClientHandlerStates::Err => {
          println!("ClientHandler: error occured!");
        },
        ClientHandlerStates::Init => {
          self.fsm.transition(ClientHandlerStates::WaitEvent.to_u8()).unwrap();
        },
        ClientHandlerStates::WaitEvent => {
          let event = match rx.recv() {
            Ok(event) => event,
            Err(_) => {
              self.fsm.transition(ClientHandlerStates::Err.to_u8()).unwrap();
              continue;
            }
          };

          println!("ClientHandler received new event: {}", event.kind.to_string());
        }
      }
    }
  }

  pub fn listener(port: u16, tx: Sender<Event>, clients: Arc<Mutex<HashMap<u8, TcpStream>>>) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();
    for stream in listener.incoming() {
      let stream = stream.unwrap();

      let mut id = tools::gen_random_id();
      while {
        let clients = clients.lock().unwrap();
        clients.contains_key(&id)
      } {
        id = tools::gen_random_id();
      }

      let mut clients = clients.lock().unwrap();
      clients.insert(id, stream);

      let event = Event::new(SelfEvents::NewConnection as u8, vec![vec![id]]);
      tx.send(event).unwrap();
    }
  }
}
