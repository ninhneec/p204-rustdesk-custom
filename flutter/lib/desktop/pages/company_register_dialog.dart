import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:http/http.dart' as http;

class CompanyRegisterDialog extends StatefulWidget {
  @override
  _CompanyRegisterDialogState createState() => _CompanyRegisterDialogState();
}

class _CompanyRegisterDialogState extends State<CompanyRegisterDialog> {
  final TextEditingController _keyController = TextEditingController();
  bool _isLoading = false;
  String _errorMessage = '';

  Future<String> _getApiBase() async {
    // Try cloud config first
    try {
      final res = await http.get(
        Uri.parse('https://raw.githubusercontent.com/ninhneec/p204-rustdesk-custom/master/cloud_config.json'),
      ).timeout(const Duration(seconds: 4));
      if (res.statusCode == 200) {
        final data = jsonDecode(res.body);
        final api = data['api_server'] as String?;
        if (api != null && api.isNotEmpty) {
          return api.endsWith('/') ? api.substring(0, api.length - 1) : api;
        }
      }
    } catch (_) { /* fallback */ }
    // Fallback to default
    return 'http://ad.apndocs.site:3000';
  }

  Future<void> _verifyKey() async {
    final key = _keyController.text.trim();
    if (key.isEmpty) {
      setState(() => _errorMessage = 'Vui lòng nhập Seat Enrollment Key');
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    try {
      final rustdeskId = gFFI.serverModel.serverId.text.replaceAll(' ', '');
      final hostname = Platform.localHostname;
      final apiBase = await _getApiBase();

      final response = await http.post(
        Uri.parse('$apiBase/api/keys/verify'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({
          'enrollment_key': key,
          'rustdesk_id': rustdeskId,
          'hostname': hostname,
        }),
      ).timeout(const Duration(seconds: 10));

      final data = jsonDecode(response.body);

      if (response.statusCode == 200 && data['success'] == true) {
        await bind.mainSetLocalOption(key: 'P204_SeatID', value: data['seat_id']);
        await bind.mainSetLocalOption(key: 'P204_Token', value: data['client_token']);

        Get.back();
        showToast('Đăng ký máy thành công. Hệ thống đã kết nối.');
      } else {
        setState(() {
          _errorMessage = data['message'] ?? 'Lỗi xác thực. Vui lòng thử lại.';
        });
      }
    } catch (e) {
      setState(() {
        _errorMessage = 'Không thể kết nối đến máy chủ P204. Lỗi: $e';
      });
    } finally {
      setState(() => _isLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return WillPopScope(
      onWillPop: () async => false,
      child: AlertDialog(
        title: Text('🏢 Đăng Ký Máy P204'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Máy này chưa được liên kết với phòng máy P204. '
                'Vui lòng nhập Seat Enrollment Key do Quản trị viên cung cấp:'),
            SizedBox(height: 16),
            TextField(
              controller: _keyController,
              decoration: InputDecoration(
                labelText: 'Seat Enrollment Key',
                border: OutlineInputBorder(),
                hintText: 'P204-XXXXXXXX',
              ),
              enabled: !_isLoading,
              textCapitalization: TextCapitalization.characters,
            ),
            if (_errorMessage.isNotEmpty) ...[
              SizedBox(height: 8),
              Text(_errorMessage, style: TextStyle(color: Colors.red, fontSize: 12)),
            ]
          ],
        ),
        actions: [
          ElevatedButton(
            onPressed: _isLoading ? null : _verifyKey,
            child: _isLoading
                ? SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                : Text('Xác Nhận & Đăng Ký'),
          )
        ],
      ),
    );
  }
}

Future<void> checkCompanyRegistration() async {
  final seatId = await bind.mainGetLocalOption(key: 'P204_SeatID');
  final token = await bind.mainGetLocalOption(key: 'P204_Token');

  if (seatId.isEmpty || token.isEmpty) {
    Get.dialog(CompanyRegisterDialog(), barrierDismissible: false);
  }
}
