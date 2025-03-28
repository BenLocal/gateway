cd ../../
cargo build
cp -rf target/debug/gateway ./test/gate/gateway
cd ./test/gate/ && docker-compose up -d --build