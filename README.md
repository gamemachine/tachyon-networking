# Tachyon

Tachyon is a performant and highly parallel reliable udp library that uses a nack based model.

* Strongly reliable
* Reliable fragmentation
* Ordered and unordered channels
* Identity management
* Suitable for high throughput IPC as well as games.

Tachyon was specifically designed for a verticaly scaled environment with many connected clients and high throughput local IPC.  Parallelism for receives was necessary to be able to process the volume within a single frame.  And larger receive windows were needed to handle the IPC volume while still retaining strong reliability.


## Reliablity
Reliability models vary a lot because there aren't any perfect solutions and context tends to matter a lot.  But a core technique that is used in a good number of them is [Glen Fiedler's approach](https://gafferongames.com/post/reliable_ordered_messages/) that encodes acks as bit flags. The most well known usage of it is to encode acks in every outgoing message.  But that means if you send 33 messages per frame, you used up the entire window in a single frame. It's designed to be used with higher level message aggregation, sending say 1-4 packets per frame or so.

The nack model can optimistically cover a much larger window in 33 slots using that same technique, because we are only covering missing packets.  Tachyon extends this by chaining multiple 33 slot nack windows over a much larger receive window as needed, the default receive window being 512 slots.

The receive window has a configurable max. It starts at the last in order sequence received, and runs to the last sequence received.  Once per frame we walk this window back to front and create a nack messages for every 33 slots.  And then pack those into a single varint encoded network packet and send it out.

But that message itself could get dropped, introducing latency.  So we also support taking those same nacks and insert them into outgoing messages in a round robin fashion. Up to ChannelConfig.nack_redundancy times per unique nack.  The cost for redundancy is the outgoing message header size goes from 4 to 10 bytes.

One thing to keep in mind is that the nack model needs a constant message flow in order to know what is missing.  So if you have channels with only occasional messages, you should send a header + 1 sized message regularly.  Tachyon should just add an internal message here that automatically sends on every channel if nothing else went out, but that's not in yet.

We also have logic to expire messages that last too long in the send buffers. Like occasional large messages that have their own channel.  The send buffer is 1024, double the size of the default receive window.


## Channels
Sequencing is per channel. With every connected address (connection) having it's own set of channels.

The system configures two channels automatically channel 1 being ordered and channel 2 unordered. And you can add more but they need to be added before bind/connect.  Because they are per address, on the server side we lazily create channels as we see receives from new addresses. 

## Fragmentation
Fragments are individually reliable.  Each one is sent as a separate sequenced message and tagged with a group id.  When the other side gets all of the fragments in the group 
we re assemble the message and deliver it.  So larger messages work fairly well, just not too large where you start chewing up too much of the receive window.

## Ordered vs Unordered
Both ordered and unordered are reliable.

Ordered messages are only delivered in order.
Unordered are delivered as soon as they arrive.

## Connection management
Tachyon connections mirror udp connections, the only identifying information is the ip address.

And then we add an Identity abstraction that can be linked to a connection.  An identity is an integer id and session id created by the application.  You set an id/session pair on the server, and you tell the client what they are out of band say via https.  If configured to use identities the client will automatically attempt to link it's identity after connect.  If the client ip changes it needs to request to be linked again.  The server when it links first removes any addresses previously linked.  With identities enabled regular messages are blocked on both ends until identity is established.


## Concurrency
Tachyon can be run highly parallel but uses no concurrency internally.  By design nothing in Tachyon is thread safe.

Parallelism is achieved by running multiple Tachyon's on different ports, and the Pool api exposes those as a single Tachyon more or less.  Managing the internal mappings of connections and identities to ports for you.  And then it runs the receives for those in parallel.

Sending unreliable messages from multiple threads is supported through special unreliable senders (see below).

Parallel receiving uses batching concurrency in it's flow.  We use a concurrent queue of non concurrent queues to limit atomic operations to just a small handful per tachyon instance.

## Unreliable senders
UnreliableSender and PoolUnreliableSender exist so you can send unreliable messages from multiple threads.  They are  intended to be used
for sending a bunch of messages with one instance, and are a bit heavy to instantiate per message.  You can create multiple of these using them in different threads,
but you can't use one concurrently from multiple threads. 

## Usage
Not much in the way of documentation yet but there are a good number of unit tests. And ffi.rs encapsulates most of the api.  tachyon_tests.rs has some stress testing unit tests.  The api is designed primarily for ffi consumption, as I use it from a .NET server.

update() has to be called once per frame.  That is where nacks and resends in response to nacks received are sent.  In addition to some housekeeping and fragment expiration.  Sends are processed immediately.

### Pool usage
The pool api has mostly the same send interface as Tachyon single usage.  Mapping of connections and identities to servers is handled internally.  So you just send to
and address/identity and the pool maps that to the right server.

There are 3 versions of the receive api currently. Two of them do heap allocations and a newer but more complex version
that does not.  That version writes out received messages into a single out buffer per tachyon, with individual messages prefixed with length, channel, and ip address.  And then you read that out buffer using LengthPrefixed like a stream.  This extra work is primarily to avoid memory fragmention from unnecessary allocations.




### Basic usage.

The default send api is send_to_target that takes a SendTarget.  If an identity id is set it will lookup the address using that.  If not will use the address.
Clients always send to a default NetworkAddress.  This api is uniform for Tachyon and Pool.

```
let config = TachyonConfig::default();
let server = Tachyon::create(config);
let client = Tachyon::create(config);

let address = NetworkAddress { a: 127, b: 0, c: 0, d: 0, port: 8001};
server.bind(address);
client.connect(address);

let send_buffer: Vec<u8> = vec![0;1024];
let receive_buffer: Vec<u8> = vec![0;4096];

let target = SendTarget {address: NetworkAddress::default(), identity_id: 0};
client.send_to_target(1, target, &mut send_buffer, 32);
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
