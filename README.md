# Tachyon

Tachyon is a performant and highly parallel reliable udp library that uses a nack based model. It provides strong reliablity, ordered and unordered channels, and reliable fragmentation.

## Reliablity
Reliabilty is based on message receive windows of a maximum size, the default being 512.  The window is defined as the last in order message through the last received message.  If the difference exceeds the max the start of the window is increased by one, and the missing packet for that slot is then lost.

The design is that nack requests are always acknowledged.  If the other side no longer has the message buffered it responds with a NONE message. But it always responds.

Nacks are only sent once per frame/update.  Here I borrowed [Glen Fiedler's approach](https://gafferongames.com/post/reliable_ordered_messages/) and encode nacks using bit flags.  I walk the receive window back to front and create a nack message for every 33 sequence numbers, and then pack all of those into a varint encoded packet.  So it's quite space efficient.  It also works similarly to Glen's approach in how many chances a packet has to be acked. But instead of being a fixed number it's based on the receive window size.

Vs a more traditional ack system it's roughly the same in terms of reliablity, but more space efficient because we only nack what is missing.  Nack models aren't widely used because they don't work for the generic case well.  The model relies on having a steady stream of messages to know what is missing. And that messages are mostly smaller and a relatively small receive window so send buffers don't grow too large..  Games have the qualities needed to make the model work well.

But you do need to be mindful of how this works, so that for every channel you have setup, you are sending a message per frame on every one. If you have an ordered channel just for large messages that's likely safe, as the fragments are themselves part of the ordered stream and will trigger nacks.  A header + 1 sized message is sufficient here, and big picture sending these keep alives is still far less bandwidth then a traditional ack model.


## Channels
Sequencing is per channel. With every connected address (connection) having it's own set of channels.  And per channel stats that are recorded.
The system configures two channels automatically channel 1 being ordered and channel 2 unordered. And you can add more but they need to be added before bind/connect.  Because they are per address, on the server side we lazily create channels as we see receives from new addresses. 

## Fragmentation
Fragments are individually reliable.  Each one is sent as a separate sequenced message.  So larger messages work fairly well, just not too large where you start chewing up too much of the receive window.

## Ordered vs Unordered
The difference here is pretty simple and as you would expect.  Ordered messages are only delivered in order. Unordered are delivered as soon as they arrive.  Both are reliable.

## Unreliable
Unreliable messages have a hot path where there is almost no processing done.  Reliable messages we have to buffer anyways, so sent/received messages you are just dealign with the body.  With unreliable you have to send a byte array that is the message plus 1 byte. And received messages will also include the 1 byte header. You don't touch the header and you can't mess it up because Tachyon will write it on send.  But you do have to reason about it.  The alternative is memcpy on send and receive.

## Connection management
This is best managed outside the library itself mostly, but I will be adding an approved address map so receives can early out.  That's about as much as I think a library at this level should be doing.

## Concurrency
The best concurrency is no concurrency. Tachyon is parallel but uses no concurrency primitives internally. Instead it leverages the nature of Udp by simply running multiple Tachyon instances one per port.  And provides a Pool abstraction to manage these instances and run their receive flows in parallel. 

Concurrent sending is supported for unreliable but not reliable. For reliable sends you have to send from one thread at a time.  Udp sockets are atomic at the OS level, so unreliable is just a direct path to that. UnreliableSender is a struct that can be created for use in other threads. That is a rust thing it's not needed for actual thread safety it's just needed for Rust to know it's safe.

Parallel receiving does have additional overhead.  It allocates bytes for received messages and then finally pushes those all to a single consumer queue. The design is a concurrent queue of non concurrent queues, so a parallel receive we do a single atomic pop to get the regular queue, and another one to push it back on the concurrent queue once receiving is done. Other then the countdown event for waiting on the parallel work that's the entirety of the concurrency.

## Performance
Cpu time is mostly in udp syscalls. But very heavy packet loss can also increase cpu time because larger receive windows make Tachyon itself do more work as it has to iterate over those.

## Usage
Not much in the way of documentation yet but there are a good number of unit tests. And ffi.rs encapsulates most of the api.  tachyon_tests.rs has some stress testing unit tests.  The api is designed primarily for ffi consumption, as I use it from a .NET server.

### Basic usage.

```
let config = TachyonConfig::default();
let server = Tachyon::create(config);
let client = Tachyon::create(config);

let address = NetworkAddress { a: 127, b: 0, c: 0, d: 0, port: 8001};
server.bind(address);
client.connect(address);

let send_buffer: Vec<u8> = vec![0;1024];
let receive_buffer: Vec<u8> = vec![0;4096];

client.send_reliable(1, NetworkAddress::default(), &mut send_buffer, 32);
let receive_result = server.receive_loop(&mut receive_buffer);
if receive_result.length > 0 && receive_result.error == 0 {
    server.send_reliable(1, receive_result.address, &mut send_buffer, 32);
}
```


### Pool usage
```
let mut pool = Pool::create();
let config = TachyonConfig::default();

// create_server also binds the port
pool.create_server(config, NetworkAddress::localhost(8001));
pool.create_server(config, NetworkAddress::localhost(8002));
pool.create_server(config, NetworkAddress::localhost(8003));

// using built in test client which removes some boilerplate
let mut client1 = TachyonTestClient::create(NetworkAddress::localhost(8001));
let mut client2 = TachyonTestClient::create(NetworkAddress::localhost(8002));
let mut client3 = TachyonTestClient::create(NetworkAddress::localhost(8003));
client1.connect();
client2.connect();
client3.connect();

let count = 20000;
let msg_len = 64;

for _ in 0..count {
    client1.client_send_reliable(1, msg_len);
    client2.client_send_reliable(1, msg_len);
    client3.client_send_reliable(1, msg_len);
}
// non blocking receive
let receiving = pool.receive();

// call this to finish/wait on the receive.
let res = pool.finish_receive();

// or alternative call pool.receive_blocking() which doesn't need a finish_receive().
```
