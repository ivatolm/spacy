use num_derive::FromPrimitive;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;
use common::{
  tools,
  message::{self, proto_msg}
};

#[derive(FromPrimitive)]
enum SelfEvents {
  NoOp = 0
}

#[pyclass]
#[derive(Clone)]
struct SpacyEvent {
  pub kind: u8,
  pub data: Vec<String>
}

#[pymethods]
impl SpacyEvent {
  #[new]
  fn new(kind: u8, data: Vec<String>) -> Self {
    Self { kind, data }
  }

  #[getter]
  fn kind(&self) -> u8 {
    self.kind
  }

  #[getter]
  fn data(&self) -> Vec<String> {
    self.data.clone()
  }
}

#[pyclass(subclass)]
struct SpacyPlugin {
  pub stream: TcpStream
}

#[pymethods]
impl SpacyPlugin {
  #[new]
  fn new() -> Self {
    let ip = tools::local_ip();
    let stream = TcpStream::connect((ip, 32001)).unwrap();
    Self { stream }
  }

  fn update(&self, _event: SpacyEvent) -> SpacyEvent {
    SpacyEvent::new(SelfEvents::NoOp as u8, vec![])
  }

  fn run(mut _self: PyRefMut<'_, Self>) {
    let mut response = SpacyEvent::new(SelfEvents::NoOp as u8, vec![]);
    let gil = Python::acquire_gil();
    let py = gil.python();
    let obj = _self.into_py(py);

    loop {
      let py_response = response.into_py(py);
      let py_request = obj.call_method1(py, "update", PyTuple::new(py, &[py_response])).unwrap();

      obj.call_method1(py, "send", PyTuple::new(py, &[py_request])).unwrap();
      response = obj.call_method1(py, "get", {}).unwrap().extract(py).unwrap();

      thread::sleep(Duration::from_secs(1));
    }
  }

  fn send(&mut self, event: SpacyEvent) {
    let msg = proto_msg::Message {
      cmd: Some(event.kind as i32),
      data: event.data
    };
    self.stream.write(&message::serialize_message(msg)).unwrap();
  }

  fn get(&mut self) -> SpacyEvent {
    let mut buf = [0u8; 16384];
    let size = self.stream.read(&mut buf).unwrap();

    let msg = message::deserialize_message(&buf[..size]).unwrap();

    SpacyEvent::new(msg.cmd.unwrap() as u8, msg.data)
  }
}

#[pymodule]
fn spacy_plugin(_py: Python, m: &PyModule) -> PyResult<()> {
  m.add_class::<SpacyEvent>()?;
  m.add_class::<SpacyPlugin>()?;
  Ok(())
}
