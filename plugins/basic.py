import sys
from os import path
container_dir = path.abspath(path.join(path.dirname(__name__), './container'))
sys.path.append(container_dir)

import spacy_plugin
import time

def bytes_to_str(bytes_vec):
    string = ""
    for byte in bytes_vec:
        string += chr(byte)
    return string

class BasicPlugin(spacy_plugin.SpacyPlugin):
    def __init__(self):
        super().__init__()
        self.iteration = 0

    def update(self):
        event = self.get_event()
        if event is not None:
            if event.kind == 123:
                print(f"Plugin has got new event: {event.kind}, {event.data}, {event.meta}")
                print(bytes_to_str(event.data[0]))
                self.respond_client([b"Hello!"], event.meta)

        if self.iteration == 0:
            some_value = 42
            b_some_value = some_value.to_bytes(4, 'little')
            self.shared_memory_push(0, b_some_value)

            self.shared_memory_get(0)

        self.iteration += 1
        time.sleep(1)

plugin = BasicPlugin()
while True:
    plugin.step()
    plugin.execute()
