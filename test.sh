#!/bin/bash

curl -X POST -H "Content-Type: rext/plain" -d 'fn main() {println!("hello world!"); }' "http://localhost:8000/execute"
