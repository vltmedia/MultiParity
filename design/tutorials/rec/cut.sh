#!/bin/bash
# Cut and export one recorded tutorial in Kitlig, then copy it into final/.  bash cut.sh t02-new-project
set -e
cd "$(dirname "$0")"
sid="$1"
python sheet.py "$sid" >/dev/null
out=$(python assemble.py "$sid" --export 2>&1 | grep '"outputs"')
p=$(echo "$out" | python -c "import sys,json;print(json.loads(sys.stdin.read())['outputs'][0]['path'])")
name=$(python -c "import re,sys;sys.path.insert(0,'.');from assemble import TITLES;s=sys.argv[1];print(s.split('-')[0]+' '+re.sub(r'[^A-Za-z0-9 ,.-]','',TITLES[s])+'.mp4')" "$sid")
cp "P:/Projects/UAI/MultiParity/Kitlig/multiparity/$p" "../final/$name"
echo "OK $sid -> final/$name"
