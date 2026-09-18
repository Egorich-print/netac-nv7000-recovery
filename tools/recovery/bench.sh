#!/bin/bash
sudo -v
MP=/mnt/2TB-NVMe; B=$MP/bench
sudo mkdir -p "$B/small"
sync
stat -f -c 'blocks=%b free=%f avail=%a bsize=%s' "$MP" 2>/dev/null || echo "stat failed"
xfs_db -r -c "freesp -s" /dev/nvme0n1p1 2>/dev/null | tail -3

echo "=== APST off ==="
sudo python3 -u "$(dirname "$0")/apst_off.py"

smart() { sudo smartctl -a /dev/nvme0 | grep -E "Temperature:|Percentage Used|Data Units Written|Data Units Read|Media and Data Integrity|Available Spare:"; }
echo "=== SMART BEFORE ==="; smart

( for i in $(seq 1 360); do sudo smartctl -a /dev/nvme0 2>/dev/null | awk -v t="$(date +%s)" '/^Temperature:/{print t, $2}'; sleep 5; done ) > /tmp/nvme_temp.log 2>&1 &
TPID=$!

run() {
  local name="$1"; shift
  echo; echo "########## $name"
  sudo fio --output-format=normal "$@" 2>&1 | grep -E "read: IOPS|write: IOPS|read: bw|write: bw|clat percentiles|lat \(usec\)|lat \(msec\)|  lat \(nsec\)|clat \(usec\)|clat \(msec\)|IOPS=" 
}

run "SEQ WRITE 1M QD32 16G" --name=seqw --directory=$B --filename=seq1m.bin --rw=write --bs=1M --iodepth=32 --ioengine=libaio --direct=1 --size=16G --group_reporting
run "SEQ READ 1M QD32 16G" --name=seqr --directory=$B --filename=seq1m.bin --rw=read --bs=1M --iodepth=32 --ioengine=libaio --direct=1 --size=16G --group_reporting
run "SEQ WRITE 128K QD16 8G" --name=seqw128 --directory=$B --filename=seq128k.bin --rw=write --bs=128k --iodepth=16 --ioengine=libaio --direct=1 --size=8G --group_reporting
run "SEQ READ 128K QD16 8G" --name=seqr128 --directory=$B --filename=seq128k.bin --rw=read --bs=128k --iodepth=16 --ioengine=libaio --direct=1 --size=8G --group_reporting
run "RAND READ 4K QD1 30s" --name=rr4k1 --directory=$B --filename=rand4k.bin --rw=randread --bs=4k --iodepth=1 --ioengine=libaio --direct=1 --size=8G --time_based --runtime=30 --group_reporting
run "RAND READ 4K QD32 30s" --name=rr4k32 --directory=$B --filename=rand4k.bin --rw=randread --bs=4k --iodepth=32 --ioengine=libaio --direct=1 --size=8G --time_based --runtime=30 --group_reporting
run "RAND WRITE 4K QD32 30s" --name=rw4k32 --directory=$B --filename=randw.bin --rw=randwrite --bs=4k --iodepth=32 --ioengine=libaio --direct=1 --size=8G --time_based --runtime=30 --group_reporting
run "MIX 70/30 4K QD32 30s" --name=mix --directory=$B --filename=mix.bin --rw=randrw --rwmixread=70 --bs=4k --iodepth=32 --ioengine=libaio --direct=1 --size=8G --time_based --runtime=30 --group_reporting
run "SMALL FILES WRITE 20k x 16K" --name=smallw --directory=$B/small --nrfiles=20000 --filesize=16k --bs=16k --rw=write --iodepth=16 --ioengine=libaio --direct=1 --group_reporting
run "SMALL FILES READ 20k x 16K" --name=smallr --directory=$B/small --nrfiles=20000 --filesize=16k --bs=16k --rw=read --iodepth=16 --ioengine=libaio --direct=1 --group_reporting

kill $TPID 2>/dev/null
echo
echo "=== TEMP MAX/AVG ==="
awk '{s+=$2; n++; if($2>m)m=$2} END{if(n)printf "samples=%d avg=%.1fC max=%dC\n", n, s/n, m}' /tmp/nvme_temp.log
echo "=== SMART AFTER ==="; smart
echo "=== dmesg ==="
sudo dmesg | grep -i -E "nvme|I/O error|reset" | tail -10
echo BENCH-DONE
