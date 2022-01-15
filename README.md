# Tachyon

Tachyon is a reliable udp library that uses a nack based model. It provides ordered and unordered channels as well as fragmentation.

## Reliablity
Reliabilty is based on receive windows and is quite robust.  Receive windows are configurable the default is set to 512 messages.  The window is defined as the last in order message through the last received message.  And the last in order is bumped up once the window reaches the max.

The design is that nack requests are always acknowledged.  If the other side no longer has the message buffered it responds with a NONE message. But it always responds.

Nacks are only sent once per frame/update.  Here I borrowed [Glen Fiedler's approach](https://gafferongames.com/post/reliable_ordered_messages/) and encode nacks using bit flags.  I walk the receive window back to front and create a nack message for every 33 sequence numbers, and then pack all of those into a varint encoded packet.  So it's quite space efficient.  The only down side to this approach is it's a single nack packet per frame, so those nack messages themselves can get lost.  But with such a large receive window it's still quite resilient even with heavy packet loss.

So worst case there are 512 nacks.  So 16 nack messages 6 bytes each for 96 bytes total. But those are varint encoded so something less then 96.  Those are all in a single packet with it's own 4 byte header.  Plus IP/UDP headers.

If you have really high packet loss you will start requesting sequences the other side doesn't have, as your own receive window is not increasing from all the lost packets.  This is basically a failed connnection as in my testing it took around 80% sustained packet loss at high message counts to start generating NONE responses.

## Fragmentation
Fragments are individually reliable.  Each one is sent as a separate sequenced message.  So larger messages work fairly well here.

## Ordered vs Unordered
The difference here is pretty simple and as you would expect.  Ordered messages are only delivered in order. Unordered are delivered as soon as they arrive.  Both are reliable.

## Unreliable
Unreliable messages have a hot path where there is almost no processing done.  Reliable messages we have to buffer anyways, so sent/received messages you are just dealign with the body.  With unreliable you have to send a byte array that is the message plus 4 bytes. And received messages will also include the 4 byte header. You don't touch the header and you can't mess it up because Tachyon will write it on send.  But you do have to reason about it.  The alternative is memcpy on send and receive.

## Performance and concurrency
Performance is fairly good but depends a lot on network packet loss.  Cpu time goes up as the receive window gets larger.  More clients will increase this also.  I have some built in tools for testing where you can configure the udp layer to drop packets.  I think around 2k clients is likely a realistic max right now.  Functionally I've tested up to 4k, I just don't think it will perform well enough at that number right now.

Which brings me to concurrency.  The patterns that can take this to where it can bury the hardware are fairly straight forward, but the Rust compiler can't reason about those. So that's my main challenge atm plus I'm fairly new to Rust.  The core design I want is batch based.  Receive from the network into a set of batches, then process those partitioned over a thread per batch.  Fairly simple flow with minimal number of atomic ops (but an additional memcpy). But to Rust its' all shared state.

In the end I might just end up solving this using multiple ports. Add a batching send/receive api and that might be the simplest route here. 

## Usage
Right now unfortunately not any documentation.  The best guide is look at tachyon_tests.rs and at ffi.rs.  Since my usage is actually all via ffi from a .NET server. 

There are two preconfigured channels, channel 1 is ordered and channel 2 is unordered.  Channel 0 is reserved.
