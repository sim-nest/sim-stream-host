# Fake host backend (descriptor)

Documents the stream host's fake backend: a modeled device inventory with a callback queue and
cassette replay, standing in for real audio hardware (`no-device`). The stream host binds real
devices and drives callbacks outside the cookbook sandbox eval stack, so the fake backend is
documented rather than run.
