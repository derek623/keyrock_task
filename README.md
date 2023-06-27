------------------------------
INSTRUCTION
------------------------------

There are 2 binaries in this project, the server and the client

1) To run the server, run the below command in the root folder

cargo run --release --bin server currency <depth> <port>

currency - a compulsary argument
depth - optional, default value is 10 if not specified
port - optional, default value is 30253 if not specified

Example:
cargo run --release --bin server ethbtc 10 30254

2) To run the client, run the below command in the root folder

cargo run --release --bin client <port>

port - optional, default value is 30253 if not specified

Example:
cargo run --release --bin client

------------------------------
Components
------------------------------

MarketDataSourceContainer:
A container that stores all the market data sources that implements the MarketDataSource traits.

Binance/Bitstamp:
Struct that implements the MarketDataSource trait, responsible for connecting to their respective source, get the order book, extract it and normalize it into an OrderBook struct. Finally it sends the order book to the channel that is connected to the aggregator.

Aggregator:
Receive order book update from market data source, aggregate the updates into one single merge order book per currency. It has a hashmap that stores the latest image of the merged order book for each currency so as to speed up the aggregate process. It doesn't matter how many maret data sources we have, every single time it tries to merge, it only compares the updated order book from an exchange with the merged order book. Finally it sends the merged order book to the channel which is connected to the grpc server.

GRPC server:
Receives merge order book from the aggregator, then sends it to the connected grpc clients.

Multi recevier channel:
A struct that is developed to enable multiple grpc clients. It stores a list of channels and each channel is connected to one client.

Utility:
It stores utilities functions.






