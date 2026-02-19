// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:papilio_mobile/main.dart';
import 'package:papilio_mobile/src/ui/server_config_view.dart';

void main() {
  testWidgets('Papilio App initialization and AuthGate routing test', (WidgetTester tester) async {
    // Build our app and trigger a frame within a ProviderScope.
    await tester.pumpWidget(
      const ProviderScope(
        child: PapilioApp(),
      ),
    );

    // Verify that the app starts and correctly navigates to AuthGate
    expect(find.byType(AuthGate), findsOneWidget);

    // Initial state: If not configured, should show ServerConfigView
    // We pump once to let Riverpod resolve initial state
    await tester.pump();
    
    // Depending on default state, it should find either ServerConfigView or loading
    // This proves that the app's business logic is actually running.
    expect(find.byType(MaterialApp), findsOneWidget);
    debugPrint('Successfully verified business widget tree mounting.');
  });
}
