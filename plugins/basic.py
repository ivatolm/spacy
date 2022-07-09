from re import S
import sys
from os import path
container_dir = path.abspath(path.join(path.dirname(__name__), './container'))
sys.path.append(container_dir)

import spacy_plugin
import time

class BasicPlugin(spacy_plugin.SpacyPlugin):
    def __init__(self):
        super().__init__()
        self.iteration = 0

    def update(self):
        print(self.get_event())

        some_value = 42
        b_some_value = some_value.to_bytes(4, 'little')

        if self.iteration == 0:
            self.shared_memory_push(0, b_some_value)
            self.shared_memory_get(0)

        self.iteration += 1
        time.sleep(1)

plugin = BasicPlugin()
while True:
    plugin.step()
    plugin.execute()
