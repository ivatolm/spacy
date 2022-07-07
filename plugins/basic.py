import sys
from os import path
container_dir = path.abspath(path.join(path.dirname(__name__), './container'))
sys.path.append(container_dir)

import spacy_plugin

class BasicPlugin(spacy_plugin.SpacyPlugin):
    def __init__(self):
        super().__init__()

plugin = BasicPlugin()
