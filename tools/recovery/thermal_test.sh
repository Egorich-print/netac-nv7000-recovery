#!/bin/bash
sudo -v
MP=/mnt/2TB-NVMe; B=$MP/bench

echo "=== shutdown history ==="
last -x reboot 2>/dev/null | head -4
sudo journalctl -b -1 --no-pager 2>/dev/null | grep -i -E "shutdown|reboot.target|systemd-shutdown" | tail -3

echo "=== cleanup old bench ==="
sudo rm -rf "$B"
sudo mkdir -p "$B"
sync

echo "=== APST off (volatile) ==="
sudo python3 -u "$(dirname "$0")/apst_off.py"

smart() { sudo smartctl -a /dev/nvme0 | grep -E "Temperature:|Percentage Used|Data Units Written"; }
echo "before:"; smart

cat > /tmp/templog.sh <<'EOF'
#!/bin/bash
for i in $(seq 1 240); do
  T=$(smartctl -a /dev/nvme0 2>/dev/null | awk '/^Temperature:/{print $2}')
  echo "$(date +%s) ${T:-NA}" >> /tmp/temp2.log
  sleep 2
done
EOF
chmod +x /tmp/templog.sh
rm -f /tmp/temp2.log
sudo /tmp/templog.sh &
TPID=$!
sleep 6
echo "logger samples so far: $(wc -l < /tmp/temp2.log)"

echo "=== 60s sustained SEQ WRITE (1M QD32, 64G loop) ==="
sudo fio --name=thermalw --directory=$B --filename=thermal.bin --rw=write --bs=1M \
  --iodepth=32 --ioengine=libaio --direct=1 --size=64G --time_based --runtime=60 \
  --group_reporting 2>&1 | grep -E "write: IOPS|write: bw"

echo "=== 60s sustained SEQ READ ==="
sudo fio --name=thermalr --directory=$B --filename=thermal.bin --rw=read --bs=1M \
  --iodepth=32 --ioengine=libaio --direct=1 --size=64G --time_based --runtime=60 \
  --group_reporting 2>&1 | grep -E "read: IOPS|read: bw"

kill $TPID 2>/dev/null
sleep 2
echo "=== TEMP during test ==="
awk '{if($2 ~ /^[0-9]+$/){s+=$2;n++; if($2>m)m=$2}} END{if(n)printf "samples=%d avg=%.1fC max=%dC\n",n,s/n,m; else print "no numeric samples"}' /tmp/temp2.log
echo "--- timeline (every ~10s) ---"
awk 'NR%5==1{print $2"C"}' /tmp/temp2.log | tr '\n' ' '; echo

echo "=== cleanup ==="
sudo rm -rf "$B"
sync
smart
df -h $MP
echo THERMAL-DONE
