"""
An HTTPS server nothing trusts, for the untrusted-certificate case.

Run in a `multiprocessing.Process` by
tests/test_navigation_failures_are_reported.py. It is a process rather than a
thread so that a handshake the client aborts cannot leave anything behind in
the test runner, and so the server dies with a `terminate()` however wedged a
half-open TLS connection has left it.

The certificate beside this file is self-signed and checked in on purpose: the
test needs a certificate no store on any machine will ever trust, and
generating one at test time would need a library the project does not depend
on, or an `openssl` binary that is not on every runner. Its private key is
public by construction and secures nothing — it is only ever served on
127.0.0.1 to a client that is expected to refuse it.

The socket speaks just enough HTTP to answer a client that somehow *did* trust
the certificate, so that outcome reads as a passing request rather than a hang.
"""

import socket
import ssl
import threading
from multiprocessing.queues import Queue
from pathlib import Path

CERTIFICATE = Path(__file__).parent / 'untrusted_certificate.pem'

ANSWER = (
    b'HTTP/1.1 200 OK\r\n'
    b'Content-Type: text/plain; charset=utf-8\r\n'
    b'Content-Length: 2\r\n'
    b'Connection: close\r\n'
    b'\r\n'
    b'hi'
)


def serve(port: 'Queue[int]') -> None:
    """
    Listen on a loopback port of the kernel's choosing and report it back.
    """
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(CERTIFICATE)

    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind(('127.0.0.1', 0))
    listener.listen(8)
    port.put(listener.getsockname()[1])

    while True:
        client, _ = listener.accept()
        threading.Thread(target=talk, args=(client, context), daemon=True).start()


def talk(client: socket.socket, context: ssl.SSLContext) -> None:
    """
    Offer the certificate, and take being hung up on as the expected answer.
    """
    try:
        with context.wrap_socket(client, server_side=True) as tls:
            _ = tls.recv(4096)
            tls.sendall(ANSWER)
    except OSError:
        # A client that refused the certificate closed the connection mid
        # handshake. That is the whole point of this server, not an error.
        pass
    finally:
        client.close()
