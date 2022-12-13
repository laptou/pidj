build:
  docker run -it --rm -v `pwd`:/app -v ~/.cargo/registry:/home/ccuser/.cargo/registry -v ~/.cargo/git:/home/ccuser/.cargo/git pidj/x-compiler:rpi3-raspbian-v1

fix:
  docker run -it --rm -v `pwd`:/app -v ~/.cargo/registry:/home/ccuser/.cargo/registry -v ~/.cargo/git:/home/ccuser/.cargo/git pidj/x-compiler:rpi3-raspbian-v1 cargo fix

copy:
  scp target/armv7-unknown-linux-gnueabihf/debug/pidj pi@iaa34.local:/home/pi/pidj

run:
  ssh -X -t -o SendEnv="RUST_LOG RUST_BACKTRACE" pi@iaa34.local DISPLAY=:0 /home/pi/pidj/pidj
