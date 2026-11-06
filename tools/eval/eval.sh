#!/bin/bash
# Số từ sai của bộ gõ (eval-session) trên bộ dev hoặc test, ba cách gõ: chuẩn / --realistic / --typos 0.05.
# Dò tham số chỉ trên dev; test chỉ để báo cáo.
# usage: tools/eval/eval.sh dev|test MODEL [tùy chọn songngu thêm]
set -e
cd "$(dirname "$0")/../.."
set=$1 model=$2
shift 2
B=target/release/songngu
if [ "$set" = dev ]; then
  names=(chatA chatB tech fineweb)
  files=(corpus/chatgen/dev_a.txt corpus/chatgen/dev_b.txt corpus/tech/dev.txt corpus/fineweb/dev.txt)
else
  names=(chat tech fineweb)
  files=(corpus/chatgen/test.txt corpus/tech/test.txt corpus/fineweb/test.txt)
fi
modes=("" "--realistic" "--typos 0.05")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
for i in "${!files[@]}"; do
  for j in "${!modes[@]}"; do
    # shellcheck disable=SC2086
    $B eval-session "${files[$i]}" --model "$model" ${modes[$j]} "$@" > "$tmp/$i.$j" 2>/dev/null &
  done
done
wait
total=0
printf "%-8s %7s %7s %9s %7s\n" "" chuẩn realistic "typo 5%" "từ"
for i in "${!files[@]}"; do
  row=()
  for j in "${!modes[@]}"; do
    n=$(sed -E 's/.*wrong ([0-9]+) .*/\1/' "$tmp/$i.$j")
    row+=("$n")
    total=$((total + n))
  done
  words=$(sed -E 's/^words ([0-9]+) .*/\1/' "$tmp/$i.0")
  printf "%-8s %7s %7s %9s %7s\n" "${names[$i]}" "${row[@]}" "$words"
done
echo "total $total"
