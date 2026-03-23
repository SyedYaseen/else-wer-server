cargo install cross
# Build
rustup target add aarch64-unknown-linux-gnu
cross build --target aarch64-unknown-linux-gnu --release

# Login0
ssh pi@192.168.1.10
regular pc pwd

# Copy
sudo rm -rf rustybookshelf.db else-wer logs

sudo systemctl stop else-wer.service

scp /home/loop/p/else-wer/else-wer-server/target/aarch64-unknown-linux-gnu/release/else-wer pi@192.168.1.10:/home/pi/

sudo systemctl restart else-wer.service

scp /home/loop/p/else-wer/else-wer-server/.env.pi pi@192.168.1.10:/home/pi

# Setup systemd
sudo nano /etc/systemd/system/else-wer.service

```
[Unit]
Description=Else-Wer
After=network.target

[Service]
User=pi
# Ensure this is the absolute path to the folder containing the binary
WorkingDirectory=/home/pi/
# Absolute path to the binary itself
ExecStart=/home/pi/else-wer
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```
sudo systemctl daemon-reload else-wer.service
sudo systemctl restart else-wer.service
sudo systemctl status else-wer.service