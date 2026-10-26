# Synthetic encrypted EWF fixture

`disk.raw` contains two 512-byte sectors: an MBR with one Linux partition and a
text marker in its second sector. It contains no acquired evidence.

`encrypted.E01` and `encrypted.E02` encode the same bytes as a two-segment EWF
image wrapped with AccessData ADCRYPT v1, AES-256-CTR, SHA-256, and 1,000 PBKDF2
iterations. The public test password is ` fixture password `, including both
spaces. Fixed salts and keys make this fixture reproducible.

Regenerate with `python3 scripts/tests/fixtures/generate_adcrypt.py`. The generator
uses Python's hashlib/HMAC/zlib and OpenSSL AES-ECB to construct the little-endian
CTR stream independently of Exhume. OpenSSL is not needed to run the tests.

The fixture exercises password input, decryption across segment boundaries,
opening from a later segment, exact RAW export, partition discovery, and rejection
of missing or incorrect credentials. It does not establish compatibility with
every acquisition-tool variant or certificate-based encryption.
