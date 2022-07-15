use std::{
    net::{TcpStream, Shutdown},
    io::Write,
    fs
};
use common::{event::{proto_msg, self}, utils};

fn get_from_user(greeter: &str) -> String {
    print!("{}", greeter);
    std::io::stdout().flush().unwrap();

    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
    line.remove(line.len() - 1);

    line
}

fn main() {
    let ips = utils::get_ipv4_ips();
    let first_ip = ips.get(0).unwrap();

    println!("Connecting to the node...");
    let mut stream = TcpStream::connect((first_ip.to_owned(), 32000)).unwrap();
    let event = proto_msg::Event {
        dir: None,
        dest: None,
        kind: proto_msg::event::Kind::MarkMeClient as i32,
        data: vec![],
        meta: vec![]
    };
    stream.write(&event::serialize(event)).unwrap();

    println!("Connected!");

    loop {
        let command = get_from_user(": ");

        let event = match command.as_str() {
            "new_plugin" => {
                let filename = get_from_user("Filename: ");
                let path = format!("plugins/{}", filename);
                let content = match fs::read_to_string(path) {
                    Ok(content) => content,
                    Err(error) => {
                        panic!("{}", error);
                    }
                };

                let name = get_from_user("Name: ");

                let event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: None,
                    kind: proto_msg::event::Kind::NewPlugin as i32,
                    data: vec![content.as_bytes().to_vec(), name.as_bytes().to_vec()],
                    meta: vec![]
                };

                event
            },
            "remove_plugin" => {
                let name = get_from_user("Name: ");

                let event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: None,
                    kind: proto_msg::event::Kind::RemovePlugin as i32,
                    data: vec![name.as_bytes().to_vec()],
                    meta: vec![]
                };

                event
            },
            "list" => {
                let event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: None,
                    kind: proto_msg::event::Kind::GetPluginList as i32,
                    data: vec![],
                    meta: vec![]
                };

                event
            },
            "new_event" => {
                let plugin_name = get_from_user("Plugin name: ");
                let event_kind = get_from_user("Event kind: ");
                let event_data = get_from_user("Event data: ");

                let mut data = vec![];
                for slice in event_data.split('%') {
                    data.push(slice.as_bytes().to_vec());
                }

                let result = event_kind.parse::<i32>();
                if let Err(error) = result {
                    println!("Maybe your input wasn't i32?");
                    panic!("{}", error);
                }

                let actual_event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: None,
                    kind: result.ok().unwrap(),
                    data,
                    meta: vec![]
                };

                let event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: None,
                    kind: proto_msg::event::Kind::NewPluginEvent as i32,
                    data: vec![plugin_name.as_bytes().to_vec(), event::serialize(actual_event)],
                    meta: vec![]
                };

                event
            },
            _ => {
                println!("Unknown command!");
                continue;
            }
        };

        stream.write(&event::serialize(event)).unwrap();
        println!("Done! Waiting for response...");

        let events = utils::read_events(&mut stream).unwrap();
        if events.len() != 0 {
            for event in events {
                println!("Kind: {}", event.kind);
                println!("Data: ");
                for item in event.data.iter() {
                    println!("{:?}", String::from_utf8_lossy(item));
                }
            }

        } else {
            println!("System disconnected");
            stream.shutdown(Shutdown::Both).unwrap();
            break;
        }
    }
}
