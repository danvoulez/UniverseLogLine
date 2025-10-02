# LogLine Documentation

This index provides a comprehensive list of all documentation for the LogLine system.

## Core Documentation

- [Architecture](./architecture.md) - System architecture and microservices implementation
- [Vision](./vision.md) - Core concept, philosophy, and goals of the LogLine system
- [Naming Conventions](./naming-conventions.md) - Standardized naming conventions for the system

## Implementation Plans

- [Modularization Plan](./modularization-plan.md) - Step-by-step plan for transforming the monolithic system into microservices
- [ID Refactoring Plan](./id-refactoring-plan.md) - Specific plan for extracting the ID service
- [Railway Deployment](./railway-deployment.md) - Guide for deploying the system on Railway

## Service Documentation

- [logline-id Service](./logline-id-service.md) - Comprehensive documentation for the ID service
- [logline-id README](./logline-id-README.md) - README template for the logline-id repository
- [logline-timeline README](./logline-timeline-README.md) - README template for the logline-timeline repository

## Legacy Documentation

The following documents are from the monolithic system and may need updating:

- [Multi-tenant Timeline](./multi-tenant-timeline.md)
- [Timeline Integration Summary](./timeline-integration-summary.md)
- [ID Integration Examples](./id-integration-examples.md)
- [ID Refactoring Details](./id-refactoring-details.md)
- [Enforcement Implementation](./enforcement-implementado.md)
- [Enforcement README](./enforcement-readme.md)
- [GitHub Push Guide](./github-push-guide.md)
- [Structure](./structure.md)
- [Projeto LogLine](./projeto-logline.md)

## Development Resources

- To-do list: See the project task list for implementation status
- GitHub repositories: All service repositories are available under the `logline` organization
- Client libraries: Each service provides client libraries for integration

## Getting Started

To get started with developing the LogLine system:

1. Review the [Architecture](./architecture.md) document
2. Follow the [Modularization Plan](./modularization-plan.md)
3. Set up the development environment for each service
4. Follow the [Naming Conventions](./naming-conventions.md) for consistency

## Contributing

We welcome contributions to both the codebase and documentation. Please follow these steps:

1. Follow the naming conventions and architectural principles
2. Create a branch for your changes
3. Submit a pull request with a clear description of your changes
4. Ensure tests pass for any code changes
5. Update documentation as needed

## Documentation Maintenance

This documentation is maintained by the LogLine team. If you find any inconsistencies or have suggestions for improvements, please submit an issue or pull request.