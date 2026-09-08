package io.sockudo.client

import java.lang.reflect.InvocationTargetException
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking

class EventLoopTest {

    @Test
    fun `subscriber callbacks run on the dedicated event thread`() = runBlocking {
        val client = testClient()
        val callbackThread = AtomicReference<String?>()
        client.bind("chat-message") { _, _ -> callbackThread.set(Thread.currentThread().name) }

        inbox(client).trySend("""{"event":"chat-message","data":{"id":42}}""")

        waitFor { callbackThread.get() != null }
        val name = callbackThread.get()!!
        assertTrue(name.startsWith("sockudo-event"), "Expected event thread, got: $name")
        client.close()
    }

    @Test
    fun `inbox overflow reports an overload error to connection listeners`() = runBlocking {
        val client = testClient()
        val errors = CopyOnWriteArrayList<SockudoError>()
        client.bindConnectionListener(
            object : SockudoConnectionEventListener {
                override fun onError(error: SockudoError) {
                    errors += error
                }
            },
        )

        onInboxOverflow(client)

        assertEquals(listOf("client_overloaded"), errors.map { it.code })
        client.close()
    }

    private fun testClient(): SockudoClient =
        SockudoClient(
            "app-key",
            SockudoOptions(
                cluster = "local",
                forceTls = false,
                enabledTransports = listOf(SockudoTransport.ws),
                wsHost = "127.0.0.1",
                wsPort = 6001,
            ),
        )

    @Suppress("UNCHECKED_CAST")
    private fun inbox(client: SockudoClient): Channel<Any> {
        val field = SockudoClient::class.java.getDeclaredField("inbox")
        field.isAccessible = true
        return field.get(client) as Channel<Any>
    }

    private fun onInboxOverflow(client: SockudoClient) {
        val method = SockudoClient::class.java.getDeclaredMethod("onInboxOverflow")
        method.isAccessible = true
        try {
            method.invoke(client)
        } catch (error: InvocationTargetException) {
            throw error.targetException
        }
    }

    private suspend fun waitFor(timeoutMs: Long = 2_000, condition: () -> Boolean) {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            if (condition()) {
                return
            }
            delay(20)
        }
        error("Timed out waiting for condition")
    }
}
