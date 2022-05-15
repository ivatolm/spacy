use crate::event::Event;

pub struct Message {
  message: Vec<String>
}

impl Message {
  pub fn to_vec(self) -> Vec<String> {
    self.message
  }

  pub fn to_string(self) -> String {
    self.message.iter()
      .enumerate()
      .map(|(i, x)| x.to_string() + if i != self.message.len() - 1 {";"} else {""})
      .collect()
  }

  pub fn skip_first(self) -> String {
    self.message.iter()
      .skip(1)
      .enumerate()
      .map(|(i, x)| x.to_string() + if i + 1 != self.message.len() - 1 {";"} else {""})
      .collect()
  }
}

impl From<&str> for Message {
  fn from(value: &str) -> Self {
    let message: Vec<String> = value
      .split(";")
      .map(|x| x.to_string())
      .collect();

    Self { message }
  }
}

impl From<Vec<String>> for Message {
  fn from(value: Vec<String>) -> Self {
    Self { message: value }
  }
}

impl From<String> for Message {
  fn from(value: String) -> Self {
    Message::from(value.as_str())
  }
}

impl From<Event> for Message {
  fn from(value: Event) -> Self {
    let mut message = vec![value.kind.to_string()];
    message.extend(value.data);

    Self { message }
  }
}
