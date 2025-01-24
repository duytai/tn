# Streamline Your AI/ML Experiments

`tn` is a powerful tool designed to simplify the management of your AI/ML experiments. It provides an intuitive way to organize, execute, and track your experiments efficiently.

## 1. Why Easy

Managing your AI/ML experiments has never been easier. Here’s what you need for an experiment:

- **YAML Configuration File:** Define all the necessary settings in an organized and readable format.
- **Python Script:** Write the code that powers your experiment.
- **Sweep Script:** Specify the number of experiments and configurations for each run using Python.

When you first run `tn`, it sweeps to generate multiple configurations. These configurations are then distributed to a fixed number of processes. To ensure efficient use of resources, the process is terminated and a new one is created, fully releasing the GPU.

### A. Dependency Injection
TODO
