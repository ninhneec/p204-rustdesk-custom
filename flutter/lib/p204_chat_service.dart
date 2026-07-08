import 'dart:io';
import 'package:socket_io_client/socket_io_client.dart' as IO;
import 'package:flutter/material.dart';
import 'package:bot_toast/bot_toast.dart';
import 'package:flutter_hbb/common.dart';

class P204ChatService {
  static final P204ChatService _instance = P204ChatService._internal();
  factory P204ChatService() => _instance;
  P204ChatService._internal();

  IO.Socket? socket;
  bool _initialized = false;

  Future<void> init() async {
    if (_initialized) return;
    _initialized = true;

    try {
      // Lấy URL API Server từ cấu hình (ví dụ: http://ad.apndocs.site:3000)
      String apiUrl = await bind.mainGetApiServer();
      
      // Fallback nếu không cấu hình
      if (apiUrl.isEmpty) {
        apiUrl = "http://ad.apndocs.site:3000";
      }

      debugPrint("P204 Chat Service connecting to: $apiUrl");

      socket = IO.io(apiUrl, IO.OptionBuilder()
          .setTransports(['websocket'])
          .disableAutoConnect()
          .build());

      socket?.onConnect((_) async {
        debugPrint('P204 Chat Service connected');
        
        final seatId = await bind.mainGetLocalOption(key: 'P204_SeatID');
        final token = await bind.mainGetLocalOption(key: 'P204_Token');
        final rustdeskId = gFFI.serverModel.serverId.text.replaceAll(' ', '');
        final hostname = Platform.localHostname;
        
        if (seatId.isNotEmpty && token.isNotEmpty) {
           socket?.emit('join-company', {
             'client_token': token,
             'seat_id': seatId,
             'rustdesk_id': rustdeskId,
             'hostname': hostname
           });
           debugPrint('P204 Chat Service emitted join-company');
        }
      });

      socket?.on('chat-message', (data) {
        debugPrint('P204 Chat Service received message: $data');
        if (data != null && data is Map) {
          final message = data['message']?.toString() ?? '';
          if (message.isNotEmpty) {
            _showNotification(message);
          }
        }
      });

      socket?.onDisconnect((_) => debugPrint('P204 Chat Service disconnected'));

      socket?.connect();
    } catch (e) {
      debugPrint("P204 Chat Service error: $e");
    }
  }

  void _showNotification(String message) {
    BotToast.showNotification(
      title: (_) => const Text(
        'Admin Message',
        style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16),
      ),
      subtitle: (_) => Text(message),
      leading: (_) => const Icon(Icons.message, color: Colors.blue),
      duration: const Duration(seconds: 15),
      dismissDirections: [DismissDirection.up, DismissDirection.horizontal],
      backgroundColor: Colors.white,
      borderRadius: 12.0,
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 24),
    );
  }
}
