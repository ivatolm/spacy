#!/bin/sh

PLUGIN_LIB_NAME=spacy_plugin

cargo build
mv target/debug/lib$PLUGIN_LIB_NAME.so target/debug/$PLUGIN_LIB_NAME.so
