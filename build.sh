#!/bin/sh

PLUGIN_LIB_NAME=spacy_plugin

mkdir container

cargo build

cp target/debug/lib$PLUGIN_LIB_NAME.so container/
mv container/lib$PLUGIN_LIB_NAME.so container/$PLUGIN_LIB_NAME.so
