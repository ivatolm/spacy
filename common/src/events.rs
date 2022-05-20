use num_derive::FromPrimitive;

// `main` and `client_handler` events
#[derive(FromPrimitive)]
pub enum MCEvents {
  AddPlugin = 16,
  NewPluginCommand = 17
}

// `main` and `node` events
#[derive(FromPrimitive)]
pub enum MNEvents {
  NewMessage = 32
}

// `main` and `plugin` events
#[derive(FromPrimitive)]
pub enum MPEvents {
  GetNodes = 64,
  NewMainCommand = 65,
  NewPluginCommand = 66
}
