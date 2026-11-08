#!/bin/sh
set -eu
: "${ORIGIN:?Set ORIGIN to the public URL of Monumla, for example https://monumla.example.com}"
./node_modules/.bin/prisma migrate deploy
exec ./node_modules/.bin/tsx --tsconfig tsconfig.server.json server.mjs
