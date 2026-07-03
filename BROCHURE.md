# sim-stream-host

In one line: It plugs SIM's live streams into the real audio and MIDI gear on your machine and across your network.

## What it gives you

This is the layer that connects SIM to the outside world of sound. When SIM needs to send or receive a stream of audio or musical notes, this part opens the connection to a real device, a software instrument, or a peer computer on your network, then keeps the packets flowing without stutters or dropped notes. It knows which devices and ports exist, picks the right place to run each piece of a stream, and reports honestly when a requested connection cannot be made instead of quietly doing something worse. A built-in stand-in device lets everything be tested and replayed without touching real hardware, so results stay repeatable.

## Why you will be glad

- Your streams reach real speakers, microphones, and MIDI instruments without hand-wiring each one.
- Audio and note packets move steadily under load, so playback stays smooth.
- The same setup runs and passes tests with no hardware attached, so behavior is predictable.

## Where it fits

SIM is a runtime that turns instructions into results across many small parts. Most of those parts stay inside the program; this one reaches out to the machine and the local network. It sits at the edge, between SIM's stream engine and the actual audio, MIDI, and peer connections, translating what SIM wants into what the hardware and the network can do. Other parts describe the stream; this part carries it to and from the world.
