use pyo3::prelude::*;
use pyo3::types::PyTuple;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;
use common::{
  tools,
  protocol::Message,
  event::{Event, EventSender, EventKind}
};

#[pyclass]
#[derive(Clone)]
struct SpacyEvent {
  pub kind: EventKind,
  pub data: Vec<String>
}

#[pymethods]
impl SpacyEvent {
  #[new]
  fn new(kind: String, data: Vec<String>) -> Self {
    let kind = EventKind::try_from(kind).unwrap();

    Self { kind, data }
  }

  #[getter]
  fn kind(&self) -> String {
    self.kind.to_string()
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
    SpacyEvent::new(EventKind::NoOp.to_string(), vec![])
  }

  fn run(mut _self: PyRefMut<'_, Self>) {
    let mut response = SpacyEvent::new(EventKind::NoOp.to_string(), vec![]);
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
    let event = Event::new(EventSender::Lb, event.kind, event.data);
    let message = Message::from(event).to_string();
    self.stream.write(message.as_bytes()).unwrap();
  }

  fn get(&mut self) -> SpacyEvent {
    let mut buf = [0u8; 16384];
    let size = self.stream.read(&mut buf).unwrap();

    let data = String::from_utf8(buf[..size].to_vec()).unwrap();
    let message = Message::from(data).to_vec();

    let cmd = message.get(0).unwrap().to_string();
    let args = match message.len() - 1 {
      0 => vec![],
      _ => message.get(1..message.len()).unwrap().to_vec()
    };

    let event_kind: EventKind = cmd.try_into().unwrap();

    SpacyEvent::new(event_kind.to_string(), args)
  }
}

#[pymodule]
fn spacy_plugin(_py: Python, m: &PyModule) -> PyResult<()> {
  m.add_class::<SpacyEvent>()?;
  m.add_class::<SpacyPlugin>()?;
  Ok(())
}
