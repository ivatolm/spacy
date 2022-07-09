use std::{
    net::TcpStream,
    io::Write,
    fs
};
use common::event::{
    self,
    proto_msg
};

fn main() {
    println!("Connecting to the node...");
    let mut stream = TcpStream::connect(("192.168.0.106", 32000)).unwrap();
    println!("Connected!");

    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();

        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        line.remove(line.len() - 1);

        // If user input is `new_plugin`
        if line == "new_plugin" {
            print!("Filename: ");
            std::io::stdout().flush().unwrap();

            let mut filename = String::new();
            std::io::stdin().read_line(&mut filename).unwrap();
            filename.remove(filename.len() - 1);

            let path = format!("plugins/{}", filename);
            let content = fs::read_to_string(path).unwrap();

            print!("Name: ");
            std::io::stdout().flush().unwrap();

            let mut name = String::new();
            std::io::stdin().read_line(&mut name).unwrap();
            name.remove(name.len() - 1);

            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::NewPlugin as i32,
                data: vec![content.as_bytes().to_vec(), name.as_bytes().to_vec()],
                meta: vec![]
            };

            stream.write(&event::serialize(event)).unwrap();

            println!("Done!");
        }

        else if line == "new_event" {
            print!("Plugin name: ");
            std::io::stdout().flush().unwrap();

            let mut plugin_name = String::new();
            std::io::stdin().read_line(&mut plugin_name).unwrap();
            plugin_name.remove(plugin_name.len() - 1);

            print!("Event id: ");
            std::io::stdout().flush().unwrap();

            let mut event_kind = String::new();
            std::io::stdin().read_line(&mut event_kind).unwrap();
            event_kind.remove(event_kind.len() - 1);

            print!("Event data: ");
            std::io::stdout().flush().unwrap();

            let mut event_data = String::new();
            std::io::stdin().read_line(&mut event_data).unwrap();
            event_data.remove(event_data.len() - 1);

            let actual_event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: event_kind.parse::<i32>().unwrap(),
                data: vec![event_data.as_bytes().to_vec()],
                meta: vec![]
            };

            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::NewPluginEvent as i32,
                data: vec![plugin_name.as_bytes().to_vec(), event::serialize(actual_event)],
                meta: vec![]
            };

            stream.write(&event::serialize(event)).unwrap();

            println!("Done!");
        }

        else {
            println!("Unknown command!");
        }
    }
}
