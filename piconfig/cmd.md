# Build
rustup target add aarch64-unknown-linux-gnu
cross build --target aarch64-unknown-linux-gnu --release

# Login0
ssh pi@192.168.1.10
regular pc pwd

# Copy
sudo rm -rf rustybookshelf.db else-wer logs
scp /home/loop/p/else-wer/else-wer-server/target/aarch64-unknown-linux-gnu/release/else-wer 

scp /home/loop/p/else-wer/else-wer-server/.env.pi pi@192.168.1.10:/home/pi