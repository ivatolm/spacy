import sys
from os import path
container_dir = path.abspath(path.join(path.dirname(__name__), './container'))
sys.path.append(container_dir)

import spacy_plugin
import pickle
import time
import traceback
import math

def bytes_to_str(bytes_vec):
    string = ""
    for byte in bytes_vec:
        string += chr(byte)
    return string


def count_primes(a, b):
    pass


class BasicPlugin(spacy_plugin.SpacyPlugin):
    COUNT_PRIMES = 10

    def __init__(self):
        super().__init__()
        self.kinds = spacy_plugin.SpacyKinds()
        self.queue = []

        self.state = 0
        self.state_data = []
        self.target_result_cnt = 0
        self.results = []
        self.timer = 0

    def update(self):
        try:
            event = self.get_event()
            if not event: return

            if self.state == 0:
                if event.kind == self.COUNT_PRIMES:
                    if len(event.data) != 1:
                        self.respond_client([b"Wrong number of argumets"], event.meta)
                        return
                    elif self.target_result_cnt != 0:
                        self.respond_client([b"Already computing"], event.meta)
                        return
                    else:
                        try:
                            n = int(bytes_to_str(event.data[0]))
                        except Exception:
                            self.respond_client([b"Failed to parse arguments"], event.meta)
                            return

                        self.queue.append(("", event.meta))

                        tasks = []
                        tasks_num = math.ceil(n / 10000)
                        for i in range(tasks_num):
                            tasks.append((count_primes, (i * 10000 + 1, (i + 1) * 10000), 10001 + i))
                        self.target_result_cnt = len(tasks)

                        self.shared_memory_get(10000)

                        self.state_data = [tasks]
                        self.state = 1

                        return
                elif event.kind == self.kinds.kind_get_from_shared_memory:
                    if len(event.data) > 0:
                        result = pickle.loads(bytes(event.data[0]))
                        if result != -1:
                            self.results.append(result)
                            print(f"Received {len(self.results)}/{self.target_result_cnt} results")

                    length = len(self.results)
                    if length != self.target_result_cnt:
                        self.shared_memory_get(10001 + length)
                    else:
                        self.timer = round(time.time() - self.timer, 3)
                        response = self.queue.pop(0)
                        message = f"Took: {self.timer} seconds | Result: {sum(self.results)}"
                        self.respond_client([bytes(message, encoding="utf-8")], response[1])
                        self.timer = 0

                        self.state = 3

                        self.shared_memory_push(10001, pickle.dumps(-1))

                    return

            if self.state == 1:
                if event.kind == self.kinds.kind_get_from_shared_memory:
                    if len(event.data) > 0:
                        tasks = pickle.loads(bytes(event.data[0]))
                    else:
                        tasks = []

                    tasks.extend(self.state_data[0])
                    self.shared_memory_push(10000, pickle.dumps(tasks))

                    self.state_data = [self.state_data[0]]
                    self.state = 2

                    return

            if self.state == 2:
                if event.kind == self.kinds.kind_transaction_failed:
                    self.shared_memory_get(10000)

                    self.state_data = [self.state_data[0]]
                    self.state = 1

                    return
                elif event.kind == self.kinds.kind_transaction_succeeded:
                    self.state_data = []
                    self.state = 0

                    self.shared_memory_get(10001)
                    self.timer = time.time()

                    return

            if self.state == 3:
                if event.kind == self.kinds.kind_transaction_succeeded:
                    if len(self.results) > 0:
                        self.results.pop()
                        length = len(self.results)
                        self.shared_memory_push(10001 + length, pickle.dumps(-1))
                    else:
                        self.target_result_cnt = 0
                        self.state = 0

                        return

                elif event.kind == self.kinds.kind_transaction_failed:
                    length = len(self.results)
                    self.shared_memory_push(10001 + length, pickle.dumps(-1))
                    return

                elif event.kind == self.COUNT_PRIMES:
                    self.respond_client([b"Already computing"], event.meta)
                    return

                return


        except Exception:
            print(traceback.print_exc())

plugin = BasicPlugin()
while True:
    plugin.step()
    plugin.execute()
    time.sleep(0.001)
