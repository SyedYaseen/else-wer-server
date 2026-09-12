MOUNTPOINT="${MOUNTPOINT:-/home/pi/drv}"

# Detect the first USB disk (e.g., sda)
DISK=$(lsblk -o NAME,TRAN,TYPE -nr | awk '$2=="usb" && $3=="disk"{print $1; exit}')

if [ -z "$DISK" ]; then
  echo "No USB disk detected."
  exit 1
fi

echo "Detected USB disk: /dev/$DISK"

# Detect the first partition of that disk (e.g., sda1)
PART=$(lsblk -o NAME,TYPE -nr | awk -v d="$DISK" '$1~("^"d"[0-9]+$") && $2=="part"{print $1; exit}')

if [ -z "$PART" ]; then
  echo "No partition found on disk /dev/$DISK."
  exit 1
fi

PART="/dev/$PART"
echo "Detected partition: $PART"

# Create mountpoint if not present
if [ ! -d "$MOUNTPOINT" ]; then
  echo "Creating mount directory $MOUNTPOINT ..."
  mkdir -p "$MOUNTPOINT"
fi

# Check if already mounted
if mountpoint -q "$MOUNTPOINT"; then
  echo "Already mounted at $MOUNTPOINT."
  exit 0
fi

# Mount
echo "Mounting $PART → $MOUNTPOINT ..."
sudo mount "$PART" "$MOUNTPOINT"

echo "Successfully mounted!"
