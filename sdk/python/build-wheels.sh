#!/bin/bash
docker build \
    --build-arg NEXT_PUBLIC_VERSION="$(cat ../../VERSION)" \
    --progress plain \
    --target artifacts \
    --output type=local,dest=. \
    -f Dockerfile \
    ../..
