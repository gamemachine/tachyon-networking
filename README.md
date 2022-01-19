# Tachyon

Tachyon is a performant and highly parallel reliable udp library that uses a nack based model. It provides strong reliablity, ordered and unordered channels, and reliable fragmentation.


## Reliablity
Reliabilty is based on receive windows that have a configurable max size, with a default of 512.  The window is defined by the last in order sequence number received (current), to the last sequence number received.  If the difference between current and last exceeds the max, current is stepped forward by one. 

Windows, send buffers, and sequences numbers are per channel not global. So a receive window of max 512 provides reliability for the last 512 messages for the specific channel.

The design is that nack requests are always acknowledged. If the other side no longer has the message buffered it responds with a NONE message. But it always responds. In practice it takes extremely high packet loss and/or latency to generate NONE messages. The send buffer is twice the size of the receive window, so you have to go back 1024 messages to get there.  For all effective purposes that's a dead connection.

Nacks are only sent once per frame/update and use a variant of [Glen Fiedler's approach](https://gafferongames.com/post/reliable_ordered_messages/) for encoding nacks using bit flags.  The receive window is walked back to front and a nack message (6 bytes) created for every 33 sequence numbers.  Those nacks are then varint encoded in a single packet.

Nack models aren't widely used because they don't work well for the generic case.  The model relies on having a steady stream of messages to know what is missing. And that messages are not too large so send buffers chew up too much memory.  Games have the qualities needed to make the model work well.

But you do need to be mindful of how this works, so that for every channel you have setup, you are sending messages regularly to keep the nack system moving.  A header + 1 sized message per frame is sufficient here, and big picture sending these keep alives are still far less bandwidth then a traditional ack model.

Tachyon's send buffers are currently hard coded to 1024, double the size of the receive window.  A downside of the nack model is send buffers have to be held on to, the sender never gets an acknowledgement for messages received by the recipient.  So to account for channels that have occasional large messages and nothing else, a timeout mechanism is also employed to expire messages that hang around too long.

For high throughput the suggested approach is use more channels.  Every channel effectively doubles your receive window capacity.  How you split it up depends.  The basic way all reliable models work is your receive window has to be big enough to handle your volume.  Ie if you are sending 32 messages a frame, and your window is 32 messages, you have no reliablity. Window implementations work differently but the core idea is still the same.    


## Channels
Sequencing is per channel. With every connected address (connection) having it's own set of channels.

The system configures two channels automatically channel 1 being ordered and channel 2 unordered. And you can add more but they need to be added before bind/connect.  Because they are per address, on the server side we lazily create channels as we see receives from new addresses. 

## Fragmentation
Fragments are individually reliable.  Each one is sent as a separate sequenced message.  So larger messages work fairly well, just not too large where you start chewing up too much of the receive window.

## Ordered vs Unordered
The difference here is pretty simple and as you would expect.  Ordered messages are only delivered in order. Unordered are delivered as soon as they arrive.  Both are reliable.

## Unreliable
Unreliable messages have a hot path where there is almost no processing done.  Reliable messages we have to buffer anyways, so sent/received messages you are just dealign with the body.  With unreliable you have to send a byte array that is the message plus 1 byte. And received messages will also include the 1 byte header. You don't touch the header and you can't mess it up because Tachyon will write it on send.  But you do have to reason about it.  The alternative is memcpy on send and receive.

## Connection management
This is best managed outside the library itself mostly, but I will be adding the ability to block by address early in the receive flow and hooks into that for extra logic people want there.  Not all games do this the same, and I don't want to bloat the core headers here.


## Concurrency
The best concurrency is no concurrency. Tachyon is parallel but uses no concurrency primitives internally. Instead it leverages the nature of Udp by simply running multiple Tachyon instances one per port.  And provides a Pool abstraction to manage these instances and run their receive flows in parallel. 

Concurrent sending is supported for unreliable but not reliable. For reliable sends you have to send from one thread at a time.  Udp sockets are atomic at the OS level, so unreliable is just a direct path to that. UnreliableSender is a struct that can be created for use in other threads. That is a rust thing it's not needed for actual thread safety it's just needed for Rust to know it's safe.

Parallel receiving does have additional overhead.  It allocates bytes for received messages and then finally pushes those all to a single consumer queue. The design is a concurrent queue of non concurrent queues. A Tachyon instance will do a concurrent dequeue of the normal queue, receive into that, and then enqueue that back onto the concurrent queue.  Batch level atomic ops nothing fine grained.  

### When to use the pool
First off if it's not obvious it's only needed server side.  You might want to always use the pool just to leverage the ability to receive non blocking and remove that part from the main thread.  But you probably need several hundred clients before that is really a win. I would start using the pool at around 400 or so based on my own testing. 

## Performance
Cpu time is mostly in udp syscalls. But very heavy packet loss can also increase cpu time because larger receive windows make Tachyon itself do more work as it has to iterate over those.

## Usage
Not much in the way of documentation yet but there are a good number of unit tests. And ffi.rs encapsulates most of the api.  tachyon_tests.rs has some stress testing unit tests.  The api is designed primarily for ffi consumption, as I use it from a .NET server.

One important note is that update() has to be called regularly as that is where nacks are generated and sent.  In addition to some housekeeping and fragment expiration.  Sends/receives are all processed immediately there is no queuing of anything there.

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

client.update();
server.update();

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
