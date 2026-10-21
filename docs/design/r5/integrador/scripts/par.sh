cd /c/Users/AdrianGonzalez/.cargo-target/r5-design
run() { name=$1; shift; node "$@" > log-$name.txt 2>&1; echo "$name exit=$?"; }
(run c390 concepts.mjs 1 es 390 light; run cen concepts.mjs 1 en 1440 light; run cdark concepts.mjs 1 es 1440 dark) &
(run d1 doors.mjs 1 1440 light 0; run d390 doors.mjs 1 390 light 0; run ddark doors.mjs 1 1440 dark 0; run drm doors.mjs 1 1440 light 1; run mdp mobiledoor.mjs 1 door_pythagoras; run mdc mobiledoor.mjs 1 door_calculus; run mdr mobiledoor.mjs 1 door_primes) &
(run kb1 kb.mjs 1 light; run kb2 kb.mjs 2 dark; run mrm map.mjs 1 1440 light explore 1; run m390 map.mjs 1 390 light explore 0; run mtech map.mjs 1 1440 dark technical 0; run expand expand.mjs 1; run expand2 expand2.mjs) &
(run c4light c4.mjs light; run c4dark c4.mjs dark; run contrast contrast.mjs 1; run veil veil.mjs; run unk unknown.mjs light; run u2 unknown2.mjs dark 1; run unkdeep unknown-deep.mjs light; run mob1 mobile.mjs 1 light; run mob2 mobile.mjs 2 dark) &
wait
echo ALLDONE
