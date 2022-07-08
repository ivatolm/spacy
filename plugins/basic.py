import sys
from os import path
container_dir = path.abspath(path.join(path.dirname(__name__), './container'))
sys.path.append(container_dir)

import spacy_plugin
import time

class BasicPlugin(spacy_plugin.SpacyPlugin):
    def __init__(self):
        super().__init__()


    def update(self):
        some_value = 42
        b_some_value = some_value.to_bytes(4, 'little')

        self.shared_memory_push(0, b_some_value)

        time.sleep(1)

plugin = BasicPlugin()
while True:
    plugin.step()
    plugin.execute()
