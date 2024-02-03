import asyncio
import p2p_helper as libp2p

def my_callback(message):
    print("Received message:", message)

async def subscribe_task():
    await libp2p.subscribe_to_messages(my_callback)

async def main():
    libp2p.generate_ed25519_keypair("a.txt")
    bn = [
        "/ip4/127.0.0.1/udp/5000/quic-v1/p2p/12D3KooWSYoEJBh6UtfAT8wdepcvH2sjVGUrSFjgsofZwvNWgFPe"
    ]
    await libp2p.init_global_p2p_network(bn, 9655, "a.txt")
    # await asyncio.sleep(3)
    # await libp2p.connect_to_peer("/ip4/127.0.0.1/udp/54705/quic-v1/p2p/12D3KooWFDE4Ki9SPkyQKoug8mmAyxLEYJ2Uni31tZMwmfQC68MB")
    await asyncio.sleep(3)
    hs = await libp2p.get_host_addrs()
    print("Hosts: ", hs)
    await asyncio.sleep(3)
    r = await libp2p.get_global_connected_peers()

    message = b"Hello, P2P Network!"
    await libp2p.publish_message(list(message))
    # 创建一个新的异步任务来订阅消息
    asyncio.create_task(subscribe_task())
    await asyncio.sleep(3)
    message = b"Hello, P2P Network11111!"
    await libp2p.publish_message(list(message))

    await asyncio.sleep(3)
    message = b"Hello, P2P 22222!"
    await libp2p.publish_message(list(message))
    await asyncio.sleep(70)  # 继续运行一段时间以接收消息
    print(123)
    print(r)
    print(456)

if __name__ == "__main__":

    asyncio.run(main())
