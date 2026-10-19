cd /c/MPM/docs/design/r4/final/scripts
(node kb.mjs 2 dark > log-kb2.txt 2>&1; node unknown2.mjs dark 2 > log-u2.txt 2>&1) &
(node map.mjs 2 390 light explore 0 > log-m390.txt 2>&1; node map.mjs 1 1440 dark technical 0 > log-mtech.txt 2>&1; node map.mjs 1 1440 light explore 1 > log-mrm.txt 2>&1; node map.mjs 2 1440 light explore 0 > log-m2.txt 2>&1) &
wait
