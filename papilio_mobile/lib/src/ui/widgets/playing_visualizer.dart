import 'package:flutter/material.dart';
import 'dart:math' as math;

class PlayingVisualizer extends StatefulWidget {
  final Color? color;
  final double size;

  const PlayingVisualizer({super.key, this.color, this.size = 20});

  @override
  State<PlayingVisualizer> createState() => _PlayingVisualizerState();
}

class _PlayingVisualizerState extends State<PlayingVisualizer> with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  final List<double> _heightFactor = [0.2, 0.8, 0.4, 0.7];

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 600),
    )..repeat(reverse: true);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = widget.color ?? Theme.of(context).colorScheme.primary;
    
    return SizedBox(
      width: widget.size,
      height: widget.size,
      child: AnimatedBuilder(
        animation: _controller,
        builder: (context, child) {
          return Row(
            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
            crossAxisAlignment: CrossAxisAlignment.end,
            children: List.generate(_heightFactor.length, (index) {
              // 使用正弦波模拟波动，减少随机数带来的不稳定性
              final variation = math.sin(_controller.value * math.pi * 2 + index) * 0.3;
              final height = (widget.size * (_heightFactor[index] + variation)).clamp(widget.size * 0.2, widget.size);
              
              return Container(
                width: widget.size / (_heightFactor.length * 2),
                height: height,
                decoration: BoxDecoration(
                  color: color,
                  borderRadius: BorderRadius.circular(widget.size / 10),
                ),
              );
            }),
          );
        },
      ),
    );
  }
}
