pub struct Event {
  pub sender: String,
  pub title: String,
  pub data: Vec<String>
}

impl Event {
  pub fn new(sender: String, title: String, data: Vec<String>) -> Self {
    Self { sender, title, data }
  }
}
