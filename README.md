------------------------------
INSTRUCTION
------------------------------

There are 2 binaries in this project, the server and the client:

**1) Server**

To run the server, run the below command in the root folder

**cargo run --release --bin server \<currency\> \<port\>**

currency - optional, default value is ethbtc.

port - optional, the default value is 30253 if not specified

Example:

cargo run --release --bin server

cargo run --release --bin server ethbtc

cargo run --release --bin server ethbtc 30254

**2) Client**

To run the client, run the below command in the root folder

**cargo run --release --bin client \<port\>**

port - optional, the default value is 30253 if not specified

Example:

cargo run --release --bin client

cargo run --release --bin client 30254

Multiple clients can be started.

------------------------------
COMPONENTS
------------------------------

**MarketDataSourceContainer:**

A container that stores all the market data sources that implement the MarketDataSource traits. It can be enhanced to control the market data source. For example, to unsubsribe a currency from all exchanges or unsubcribe all messages from a certain exchange. It acts like a manager of all the market data sources.

**Binance/Bitstamp:**

Struct that implements the MarketDataSource trait, responsible for connecting to their respective source, getting the order book, extracting it and normalizing it into an OrderBook struct. Finally, it sends the order book to the channel that is connected to the aggregator.

**Aggregator:**

Receive order book updates from market data sources, and aggregate the updates into one single merge order book per currency. It has an array that stores the latest orderbook of each exchange. It does the merge by putting the top bid/ask of each exchange into a min-max-heap, find out the biggest/smallest with a customized PartialEq trait, then pop and put the value into the result array. Afterwards, it put the next bid/ask of the same exchange and restart the whole process. It repeats the same process until the resulting Arrayvec is full (10 entries). Finally, it sends the merged order book to the channel which is connected to the grpc server.

**GRPC server:**

Receives merge order book from the aggregator, then sends it to the connected grpc clients.

**Multi recevier channel:**

A struct that is developed to enable multiple grpc clients. It stores a list of channels and each channel is connected to one client.

------------------------------
log file
------------------------------

Log file is being written to \<basedir\>/target/logs. It rotates every 50 MB and the latest one is called server.log

------------------------------
POSSIBLE IMPROVEMENTS
------------------------------

- The server only support one currency. I thought of allowing it to support multiple currency. However, if we connect to more exchange, then supporting multiple currencies could be an issue as the load of the server maybe too much. Another solution is to deploy an instance per currency. However, we can't tell for sure which is a better choice until we can do actual measurement. Therefore, I go with the one currency solution.
- Add more test cases and also integration tests
- Add reconnection/retry logic for each source



