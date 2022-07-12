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
    GET_FROM_SHARED_MEMORY = 9
    TRANSACTION_SUCCEEDED = 14
    TRANSACTION_FAILED = 15
    SAVE_DATA = 100
    GET_DATA = 101

    def __init__(self):
        super().__init__()
        self.queue = []

    def update(self):
        try:
            event = self.get_event()
            if not event: return

            if event.kind == self.TRANSACTION_SUCCEEDED:
                response = self.queue.pop(0)

                self.respond_client([bytes("Your data was saved", encoding="utf-8")], response[1])

            elif event.kind == self.TRANSACTION_FAILED:
                response = self.queue.pop(0)

                self.respond_client([bytes("Failed to process request", encoding="utf-8")], response[1])


            elif event.kind == self.GET_FROM_SHARED_MEMORY:
                data = event.data[0]

                response = self.queue.pop(0)

                message = response[0].format(bytes_to_str(data))
                print(response, message)
                self.respond_client([bytes(message, encoding="utf-8")], response[1])

            elif event.kind == self.SAVE_DATA:
                if len(event.data) != 2:
                    self.respond_client([b"Wrong number of arguments"], event.meta)
                    self.waiting_for_data = True
                else:
                    key, value = event.data

                    int_key = int(bytes_to_str(key))

                    self.shared_memory_push(int_key, value)
                    self.queue.append(("Save data: {}", event.meta))

            elif event.kind == self.GET_DATA:
                if len(event.data) != 1:
                    self.respond_client([b"Wrong number of arguments"], event.meta)
                else:
                    key = event.data[0]

                    int_key = int(bytes_to_str(key))

                    self.shared_memory_get(int_key)
                    self.queue.append(("Get data: {}", event.meta))

            else:
                self.respond_client([b"Unknown command!"], event.meta)
        except Exception as e:
            print(e)

plugin = BasicPlugin()
while True:
    plugin.step()
    plugin.execute()
    time.sleep(1)
