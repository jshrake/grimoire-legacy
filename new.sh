#!/usr/bin/env bash

set -x
set -e

mkdir -p $1
cp -r ./examples/shadertoy-new "$1"/
