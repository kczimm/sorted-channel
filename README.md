# sorted-channel

[![codecov](https://codecov.io/github/kczimm/sorted-channel/branch/master/graph/badge.svg?token=4QTHGUQ5T0)](https://codecov.io/github/kczimm/sorted-channel)

A multi-producer, multi-consumer channel that outputs sorted messages. Each message is received by only one receiving channel.

## Examples
```rust
use sorted_channel::sorted_channel;
use std::thread;

let (tx, rx) = sorted_channel();

let handle = thread::spawn(move || {
    for i in [0, 9, 1, 8, 2, 7, 3, 6, 4, 5] {
        tx.send(i).unwrap();
    }
});

handle.join().unwrap();

for i in (0..10).rev() {
    assert_eq!(rx.recv(), Ok(i));
}
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.