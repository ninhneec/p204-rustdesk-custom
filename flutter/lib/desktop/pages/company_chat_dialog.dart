import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/ffi.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';

class CompanyChatDialog extends StatefulWidget {
  @override
  _CompanyChatDialogState createState() => _CompanyChatDialogState();
}

class _CompanyChatDialogState extends State<CompanyChatDialog> {
  final TextEditingController _msgController = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  List<Map<String, dynamic>> _messages = [];

  @override
  void initState() {
    super.initState();
    // Listen for incoming chat messages via global event channel.
    // In Rust, we push events named "company_chat".
    platformFFI.registerEventHandler('company_chat', 'company_chat_dialog', (event) async {
      if (mounted) {
        setState(() {
          _messages.add(event);
        });
        _scrollToBottom();
      }
    });
  }

  @override
  void dispose() {
    platformFFI.unregisterEventHandler('company_chat', 'company_chat_dialog');
    super.dispose();
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: Duration(milliseconds: 300),
          curve: Curves.easeOut,
        );
      }
    });
  }

  void _sendMessage() {
    final text = _msgController.text.trim();
    if (text.isEmpty) return;
    
    // Call Rust to send the chat message to the management server
    bind.companySendChat(text: text);
    
    _msgController.clear();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text('🏢 P204 Live Chat'),
      content: SizedBox(
        width: 400,
        height: 500,
        child: Column(
          children: [
            Expanded(
              child: Container(
                decoration: BoxDecoration(
                  border: Border.all(color: Colors.grey.withOpacity(0.3)),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: ListView.builder(
                  controller: _scrollController,
                  padding: EdgeInsets.all(8),
                  itemCount: _messages.length,
                  itemBuilder: (context, index) {
                    final msg = _messages[index];
                    final isMe = msg['sender'] == 'me';
                    return Align(
                      alignment: isMe ? Alignment.centerRight : Alignment.centerLeft,
                      child: Container(
                        margin: EdgeInsets.symmetric(vertical: 4),
                        padding: EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                        decoration: BoxDecoration(
                          color: isMe ? Colors.blueAccent.withOpacity(0.2) : Colors.grey.withOpacity(0.2),
                          borderRadius: BorderRadius.circular(8),
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              msg['sender'] ?? 'Unknown',
                              style: TextStyle(fontWeight: FontWeight.bold, fontSize: 12, color: isMe ? Colors.blue : Colors.orange),
                            ),
                            SizedBox(height: 4),
                            Text(msg['message'] ?? ''),
                            SizedBox(height: 2),
                            Text(
                              msg['time'] ?? '',
                              style: TextStyle(fontSize: 10, color: Colors.grey),
                            ),
                          ],
                        ),
                      ),
                    );
                  },
                ),
              ),
            ),
            SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _msgController,
                    decoration: InputDecoration(
                      hintText: 'Nhập tin nhắn...',
                      border: OutlineInputBorder(),
                      contentPadding: EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                    ),
                    onSubmitted: (_) => _sendMessage(),
                  ),
                ),
                SizedBox(width: 8),
                IconButton(
                  icon: Icon(Icons.send, color: Colors.blue),
                  onPressed: _sendMessage,
                )
              ],
            )
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Get.back(),
          child: Text('Đóng'),
        )
      ],
    );
  }
}
