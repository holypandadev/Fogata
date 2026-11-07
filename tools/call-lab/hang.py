"""A sound server that takes every connection and never answers, at the socket named, for
mic.sh's check of a server too slow to answer. Usage: hang.py <socket>"""

import os
import socket
import sys

path = sys.argv[1]
if os.path.exists(path):
    os.unlink(path)
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen(16)
held = []
while True:
    connection, _ = server.accept()
    held.append(connection)
