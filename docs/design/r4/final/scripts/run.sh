cd /c/MPM/docs/design/r4/final/scripts
(node home.mjs 2 > log-home2.txt 2>&1; node concepts.mjs 2 es 1440 light > log-c2.txt 2>&1; node concepts.mjs 1 es 390 light > log-c390.txt 2>&1; node concepts.mjs 1 en 1440 light > log-cen.txt 2>&1; node concepts.mjs 1 es 1440 dark > log-cdark.txt 2>&1) &
(node doors.mjs 2 1440 light 0 > log-d2.txt 2>&1; node doors.mjs 1 390 light 0 > log-d390.txt 2>&1; node doors.mjs 2 390 dark 0 > log-d390d.txt 2>&1; node doors.mjs 1 1440 dark 0 > log-ddark.txt 2>&1; node doors.mjs 1 1440 light 1 > log-drm.txt 2>&1; node doors.mjs 2 1440 light 1 > log-drm2.txt 2>&1) &
wait
