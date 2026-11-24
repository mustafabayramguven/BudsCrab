package dev.budscrab.app

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager

class BudsCrabApp : Application() {
    override fun onCreate() {
        super.onCreate()
        Buds.init(this)
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CH_SERVICE, "Kulaklık bağlantısı", NotificationManager.IMPORTANCE_LOW).apply {
                description = "Pil ve gürültü kontrolü kısayolları"
                setShowBadge(false)
            }
        )
        nm.createNotificationChannel(
            NotificationChannel(CH_ALERTS, "Uyarılar", NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = "Düşük pil ve benzeri uyarılar"
            }
        )
    }

    companion object {
        const val CH_SERVICE = "service"
        const val CH_ALERTS = "alerts"
    }
}
