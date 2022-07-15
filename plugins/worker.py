import sys
from os import path
container_dir = path.abspath(path.join(path.dirname(__name__), './container'))
sys.path.append(container_dir)

import spacy_plugin
import pickle
import time
import traceback
import random

def bytes_to_str(bytes_vec):
    string = ""
    for byte in bytes_vec:
        string += chr(byte)
    return string


def count_primes(a, b):
    result = 0
    for num in range(max(a, 2), b + 1):
        for i in range(2, int(num ** 0.5) + 1):
            if num % i == 0:
                break
        else:
            result += 1
    return result


class BasicPlugin(spacy_plugin.SpacyPlugin):
    COUNT_PRIMES = 10

    def __init__(self):
        super().__init__()
        self.kinds = spacy_plugin.SpacyKinds()
        self.queue = []

        self.state = 0
        self.state_data = []

        self.shared_memory_get(10000)


    def update(self):
        try:
            event = self.get_event()
            if not event: return


            if self.state == 0:
                # print(0)
                if event.kind == self.kinds.kind_get_from_shared_memory:
                    if len(event.data) == 0:
                        self.shared_memory_get(10000)
                    else:
                        self.state_data = [event.data]
                        self.state = 1

            if self.state == 1:
                # print(1)
                tasks = pickle.loads(bytes(self.state_data[0][0]))
                if len(tasks) > 0:
                    task = tasks.pop(min(random.randrange(0, 5), len(tasks) - 1))
                    new_tasks = pickle.dumps(tasks)
                    self.shared_memory_push(10000, new_tasks)

                    self.state_data = [task]
                    self.state = 2

                    return
                else:
                    self.shared_memory_get(10000)

                    self.state_data = []
                    self.state = 0

                    return

            if self.state == 2:
                # print(2)
                if event.kind == self.kinds.kind_transaction_failed:
                    self.shared_memory_get(10000)

                    self.state_data = []
                    self.state = 0

                    return
                elif event.kind == self.kinds.kind_transaction_succeeded:
                    func, args, dest = self.state_data[0]
                    result = func(*args)
                    self.shared_memory_push(dest, pickle.dumps(result))

                    self.state_data = [dest, result]
                    self.state = 3

                    return

            if self.state == 3:
                # print(3)
                if event.kind == self.kinds.kind_transaction_failed:
                    self.shared_memory_push(self.state_data[0], pickle.dumps(self.state_data[1]))

                    self.state_data = [self.state_data[0], self.state_data[1]]

                    return
                elif event.kind == self.kinds.kind_transaction_succeeded:
                    self.shared_memory_get(10000)

                    self.state_data = []
                    self.state = 0

                    return

        except Exception:
            print(traceback.print_exc())

plugin = BasicPlugin()
while True:
    plugin.step()
    plugin.execute()
    time.sleep(0.001)
